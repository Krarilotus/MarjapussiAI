"""Git Bash entry point for local, resource-bounded policy comparisons."""

import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import queue
import shlex
import subprocess
import sys
import tempfile
import threading
import time

from build import POLICIES, prepare
from report import compact_game, write_reports
from resources import Resources, RunLock

ROOT = Path(__file__).resolve().parents[2]


def install(lock):
    destination = Path.home() / ".local/bin/marja-bench"
    marker = "# Marja benchmark launcher"
    if destination.exists() and marker not in destination.read_text(encoding="utf-8"):
        raise RuntimeError(f"Refusing to replace an existing command: {destination}")
    destination.parent.mkdir(parents=True, exist_ok=True)
    source = (ROOT / "scripts/marja_bench").as_posix()
    lock_setting = f'export MARJA_BENCH_LOCK={shlex.quote(lock.resolve().as_posix())}\n'
    destination.write_text(f'#!/usr/bin/env bash\n{marker}\n{lock_setting}exec python -X utf8 {shlex.quote(source)} "$@"\n', encoding="utf-8")
    rc = Path.home() / ".bashrc"
    content = rc.read_text(encoding="utf-8") if rc.exists() else ""
    path_line = 'export PATH="$HOME/.local/bin:$PATH"'
    if path_line not in content:
        with rc.open("a", encoding="utf-8") as f:
            f.write(f"\n{marker}\n{path_line}\n")
    print(f"Installed {destination}\nIn this Git Bash window run: source ~/.bashrc\nThen: marja-bench new old new old 4000")


def load_games(directory):
    rows = []
    for path in sorted(Path(directory).glob("worker-*.ndjson")):
        with path.open(encoding="utf-8") as f:
            for line in f:
                rows.append(compact_game(json.loads(line)))
    return rows


def save_manifest(directory, manifest):
    temporary = directory / ".manifest.json.tmp"
    try:
        temporary.write_text(json.dumps(manifest, indent=2), encoding="utf-8")
        os.replace(temporary, directory / "manifest.json")
    finally:
        temporary.unlink(missing_ok=True)


def stop_workers(processes, threads):
    """Reap processes, then confirm EOF readers have closed their output files."""
    errors = []
    for process in processes:
        try:
            if process.poll() is None:
                process.terminate()
        except OSError as error:
            errors.append(str(error))
    for process in processes:
        try:
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
        except (OSError, subprocess.TimeoutExpired) as error:
            errors.append(str(error))
    deadline = time.monotonic() + 5
    for thread in threads:
        try:
            thread.join(timeout=max(0, deadline - time.monotonic()))
            if thread.is_alive():
                errors.append("Worker output writer did not stop after process termination")
        except RuntimeError as error:
            errors.append(str(error))
    if errors:
        raise RuntimeError("Worker teardown failed; raw files were not reread: " + "; ".join(errors))


def generate_reports(directory, manifest, final_status):
    """Stage all reports; publish each atomically, summary and manifest last."""
    manifest.update(status="reporting", run_status=final_status, report_status="pending")
    try:
        save_manifest(directory, manifest)
        rows = load_games(directory)
        candidate = dict(manifest, status=final_status, report_status="complete")
        candidate.pop("report_error", None)
        with tempfile.TemporaryDirectory(prefix=".report-", dir=directory) as temporary:
            staging = Path(temporary)
            result = write_reports(staging, rows, candidate)
            if final_status == "complete" and not result.get("complete"):
                raise RuntimeError("Report did not validate the completed fixed sample")
            for name in ("report.md", "report.html", "games.csv", "summary.json"):
                os.replace(staging / name, directory / name)
        manifest.update(candidate)
        manifest.pop("report_error", None)
        save_manifest(directory, manifest)
        return rows
    except BaseException as error:
        manifest.update(status="report_failed", report_status="failed", report_error=str(error))
        try:
            save_manifest(directory, manifest)
        except OSError as save_error:
            print(f"Could not save report failure status: {save_error}", file=sys.stderr)
        raise


def report_existing(directory, lock):
    # The per-run lock also prevents races when callers select different global locks.
    with RunLock(lock), RunLock(directory / ".run.lock"):
        manifest = json.loads((directory / "manifest.json").read_text(encoding="utf-8"))
        if manifest.get("status") not in ("complete", "interrupted_or_failed", "report_failed"):
            raise RuntimeError("Run is active or not finished; report-only requires a stopped run")
        if manifest.get("writers_stopped") is False:
            raise RuntimeError("Writer shutdown was not confirmed; refusing to reread raw files")
        final_status = manifest.get("run_status", manifest["status"])
        if final_status not in ("complete", "interrupted_or_failed"):
            raise RuntimeError("Run completion status is unknown; refusing to label a report complete")
        generate_reports(directory, manifest, final_status)


def run_workers(binary, options, resources, directory, manifest):
    events = queue.Queue()
    processes, threads, rows = [], [], []

    def read_worker(index, process):
        try:
            with (directory / f"worker-{index}.ndjson").open("x", encoding="utf-8") as output:
                for line in process.stdout:
                    row = json.loads(line)
                    output.write(line)
                    output.flush()
                    events.put(("game", (row.get("schema"), row["index"])))
            code = process.wait()
            events.put(("exit", (index, code)))
        except Exception as e:
            events.put(("error", f"Worker {index}: {e}"))

    start, last_progress, exited = time.monotonic(), 0, 0
    manifest.update(status="running", games_complete=False, writers_stopped=False)
    save_manifest(directory, manifest)
    failure = None
    try:
        for worker in range(options.workers):
            command = [str(binary), "--games", str(options.games), "--seed", str(options.seed),
                       "--worker-index", str(worker), "--workers", str(options.workers)]
            for seat, policy in enumerate(options.policies):
                command += [f"--seat{seat}", policy]
            with (directory / f"worker-{worker}.stderr.log").open("xb") as stderr:
                process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=stderr,
                                           text=True, encoding="utf-8", cwd=ROOT,
                                           creationflags=subprocess.CREATE_NO_WINDOW)
            processes.append(process)
            resources.pin(process, worker, options.workers)
            thread = threading.Thread(target=read_worker, args=(worker, process), daemon=True)
            threads.append(thread)
            thread.start()
        while exited < options.workers:
            try:
                event, data = events.get(timeout=1)
            except queue.Empty:
                continue
            if event == "game":
                if data[0] != "marjapussi.selfplay/1":
                    raise RuntimeError("Unexpected worker record schema")
                rows.append(data[1])
            elif event == "exit":
                worker, code = data
                if code:
                    raise RuntimeError(f"Worker {worker} exited {code}; see worker-{worker}.stderr.log")
                exited += 1
            else:
                raise RuntimeError(data)
            now = time.monotonic()
            if now - last_progress >= 10 or exited == options.workers:
                elapsed = now - start
                remaining = elapsed * (options.games / len(rows) - 1) if rows else None
                eta = f" · about {remaining / 60:.1f} min remaining" if remaining is not None else ""
                print(f"{len(rows):,}/{options.games:,} games · {elapsed:.1f}s{eta}", flush=True)
                last_progress = now
                (directory / "progress.json").write_text(json.dumps({"completed": len(rows), "requested": options.games,
                                                                     "seconds": elapsed, "remaining_seconds": remaining}))
        if sorted(rows) != list(range(options.games)):
            raise RuntimeError("Finished game indices do not match the requested fixed sample")
        manifest["games_complete"] = True
    except BaseException as error:
        failure = error
    finally:
        try:
            stop_workers(processes, threads)
            manifest["writers_stopped"] = True
        except BaseException as error:
            if failure is None:
                failure = error
            else:
                print(f"Worker cleanup also failed: {error}", file=sys.stderr)
        manifest["elapsed_seconds"] = time.monotonic() - start
        manifest["resources"] = resources.describe()
        final_status = "complete" if failure is None else "interrupted_or_failed"
        manifest.update(status="reporting" if failure is None else final_status, run_status=final_status)
        # Release indices before reloading compact report inputs from closed files.
        rows.clear()
        try:
            if manifest["writers_stopped"]:
                rows = generate_reports(directory, manifest, final_status)
            else:
                manifest["report_status"] = "skipped_writers_active"
                save_manifest(directory, manifest)
        except BaseException as error:
            if failure is None:
                failure = error
            else:
                print(f"Partial report also failed: {error}", file=sys.stderr)
    if failure is not None:
        raise failure
    return rows


def main():
    p = argparse.ArgumentParser(description="Four policy names in slot order 0 1 2 3, followed by the number of finished games.",
                                epilog="Example: marja-bench new old new old 4000 --workers 8 --cpus 16")
    p.add_argument("lineup", nargs="*", metavar="POLICY0 POLICY1 POLICY2 POLICY3 GAMES")
    p.add_argument("--seed", type=int, default=20261011)
    p.add_argument("--workers", type=int, default=1)
    p.add_argument("--cpus", type=int, default=2, help="Logical CPU limit for the whole job (default: shared-PC limit 2)")
    p.add_argument("--memory-gib", type=int, default=4)
    p.add_argument("--out", type=Path)
    p.add_argument("--cache", type=Path, default=ROOT / "target/marja-bench")
    p.add_argument("--lock", type=Path, default=Path(os.environ.get("MARJA_BENCH_LOCK", ROOT / "target/validation.lock")))
    p.add_argument("--list-policies", action="store_true")
    p.add_argument("--install", action="store_true")
    p.add_argument("--prepare", action="store_true", help="Build/cache the worker without playing games")
    p.add_argument("--report-only", type=Path, help="Regenerate a report from an existing run")
    a = p.parse_args()
    if a.list_policies:
        for name, description in POLICIES.items():
            print(f"{name:12} {description}")
        return
    if a.install:
        install(a.lock)
        return
    if a.report_only:
        report_existing(a.report_only, a.lock)
        print(a.report_only / "report.html")
        return
    if len(a.lineup) != 5:
        p.error("give four policy names and a game count; use --list-policies to see names")
    a.policies = a.lineup[:4]
    try:
        a.games = int(a.lineup[4])
    except ValueError:
        p.error("game count must be a positive integer")
    if a.games < 1 or a.games > 10_000_000 or not 0 <= a.seed < 2**63:
        p.error("games must be 1..10000000 and seed 0..2^63-1")
    if not 1 <= a.workers <= a.cpus or a.memory_gib < 1:
        p.error("workers must be between 1 and --cpus; memory must be positive")
    if a.workers > (a.games + 3) // 4:
        p.error("use no more workers than distinct deals (ceil(games/4))")
    unknown = set(a.policies) - set(POLICIES) - {"hp1"}
    if unknown:
        p.error(f"unknown policies: {', '.join(sorted(unknown))}; use --list-policies")
    stamp = datetime.now().strftime("%Y%m%d-%H%M%S-%f")
    a.out = (a.out or ROOT / "target/bench-results" / stamp).resolve()
    with RunLock(a.lock):
        resources = Resources(a.cpus, a.memory_gib)
        if a.workers > len(resources.cores):
            print("Note: more workers than allocated physical cores; latency and deadline fallbacks can change.", flush=True)
        a.out.mkdir(parents=True, exist_ok=False)
        print(f"Preparing release worker (cached builds are reused). Results: {a.out}", flush=True)
        binary, build = prepare(ROOT, a.cache, a.policies, a.out / "build.log")
        manifest = {"schema": "marjapussi.benchmark/1", "status": "prepared", "policies": a.policies,
                    "games": a.games, "seed": a.seed, "workers": a.workers,
                    "distinct_deals": (a.games + 3) // 4, "full_rotation_blocks": a.games // 4,
                    "partial_block_games": a.games % 4, "rotation": "slots_clockwise_0_1_2_3; hands_turned_two_seats_for_2_3",
                    "started_utc": datetime.now(timezone.utc).isoformat(), "resources": resources.describe(),
                    "build": build, "command": sys.argv, "sequential_stopping": False,
                    "primary_metric": "engine score difference per game, team slots0/2 minus slots1/3",
                    "uncertainty": "deal-clustered approximate normal interval",
                    "ceilings": "card+last-trick140, pairs280; scoring ceilings, not attainability"}
        save_manifest(a.out, manifest)
        if a.prepare:
            print(f"Ready: {binary}")
            return
        with RunLock(a.out / ".run.lock"):
            run_workers(binary, a, resources, a.out, manifest)
        print((a.out / "report.md").read_text(encoding="utf-8"))
        print(f"Open report: {a.out / 'report.html'}\nRaw games: {a.out}\nSummary: {a.out / 'summary.json'}")


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("Stopped. See the output manifest for worker shutdown and partial report status.", file=sys.stderr)
        sys.exit(130)
    except (RuntimeError, ValueError, OSError, subprocess.CalledProcessError) as error:
        print(f"marja-bench: {error}", file=sys.stderr)
        sys.exit(1)
