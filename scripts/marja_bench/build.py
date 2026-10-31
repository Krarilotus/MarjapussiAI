"""Build the evaluation worker; historical policies stay in an external Cargo cache."""

import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tomllib

from resources import RunLock

CACHE_LOCK = ".build.lock"
WORKER_BINARY = "marja-selfplay.exe" if os.name == "nt" else "marja-selfplay"

OLD = "1632af833199e5253271d493af8531badfac81c0"
COMPATIBLE_CORE = "b3abdaf1f5e47111904db442ce36db7dc4ae9e0b"
V1_BASELINE = "af55bedcebe982644b937573ee739dada254b9fd"
# The published v1/main head (2026-10-14): the frontier baseline of the 2026-10-31 hill climb.
V1_MAIN = "4a1c51c671db5c5d89b23ec0dc1526449c2b0dcb"
# v1/main with the deadline switch only: the reproducible baseline (`--deadlines off`).
BASE = "afe2ebb9e79eba9661c6cfdae101570578d42475"
# Checkpoint 2 (2026-10-31): budgeted seven-trick endgame, faster solver, series raise.
C2 = "1b24987b40ec70605cd8b962a9f3537020d9fe59"
HISTORICAL = {"old": OLD, "previous": COMPATIBLE_CORE, "v1base": V1_BASELINE, "v1": V1_MAIN, "base": BASE, "c2": C2}
# Source-reviewed additive knowledge helpers; existing core algorithms unchanged.
# Keep this exact anchor so any subsequent core change requires another review.
COMPATIBLE_CORES = (
    COMPATIBLE_CORE,
    "bcf567164d142c969ace2ed63b14e0e6ddedde2e",
    "4bff913bead8f33bcdce27751cfa0cb6e2296e71",
    # Additive public outcome envelope; review: 2026-10-11-public-envelope-core-anchor.md.
    "043c3ff60887782bd98c1b0b9510dfe1522e5be7",
    # Additive `knowledge::control_run` (2026-10-13): a new pure hand fact and its export; the
    # historical policies don't call it and no existing knowledge code changed.
    "5ec49bafffd30f830a17824d2c63dfc600f7f237",
    # The published v1/main core; the earlier anchors are not in every clone.
    V1_MAIN,
    # Additive (2026-10-31): SeriesScore::bonus_team, SeriesTally::resume and
    # CARD_POINTS_TOTAL. Frozen policies neither construct nor read them.
    "d3152f6f2084d5c45d81c7ea252154d73359db5e",
    # Additive (2026-10-31): FastState::question_activates.
    "19936afcfb2e71e463e6bf14bdf02167c90fca06",
    # Refactor (2026-10-31): FastState::answer_makes_trump (private), same transitions.
    "2a0343782f2572d35fa3a26261f96ddd3c8b89ce",
)
SHARED = ("engine", "observation", "knowledge", "agent")
FROZEN = ("heuristic", "beliefs", "solver")
POLICIES = {
    "new": "Current HP1, full beliefs and continuation planning (alias: hp1)",
    "old": f"Exact previous heuristic and beliefs, {OLD[:12]}",
    "previous": f"Verified live HP1 with frozen beliefs and solver, {COMPATIBLE_CORE[:12]}",
    "v1base": f"Frozen v1 handoff before the control/reuse fixes, {V1_BASELINE[:12]}",
    "v1": f"Frozen published v1/main head, {V1_MAIN[:12]}",
    "base": f"Frozen v1/main with the deadline switch (reproducible baseline), {BASE[:12]}",
    "c2": f"Checkpoint 2: budgeted seven-trick endgame, faster solver, series raise, {C2[:12]}",
    "hp1@FILE": "Current HP1 with the flat TOML overrides of FILE (parameter A/B variant)",
    "rev:COMMIT": "The heuristic, beliefs and solver frozen at a local COMMIT (shared core must be compatible)",
    "hp1-fast": "Current HP1 with continuation, return and endgame search disabled",
    "hp1-facts": "Current HP1 using hard facts without convention signals",
    "random": "Uniform random legal actions",
}


def git(repo, *args):
    return subprocess.check_output(["git", "-C", str(repo), *args], stderr=subprocess.PIPE)


def fingerprint(repo):
    names = git(repo, "ls-files", "-z", "Cargo.toml", "Cargo.lock", "crates").decode().split("\0")
    # Include new worker files while developing; committed runs also capture the revision.
    names += git(repo, "ls-files", "--others", "--exclude-standard", "-z", "crates").decode().split("\0")
    h = hashlib.sha256()
    for name in sorted(set(n for n in names if n.endswith((".rs", ".toml", ".lock")))):
        h.update(name.encode())
        h.update(b"\0")
        h.update((repo / name).read_bytes())
    return h.hexdigest()


def toolchain(repo):
    """Record versions and hash effective local build configuration without secrets."""
    configs = set()
    for parent in (repo, *repo.parents):
        configs.update(p for p in (parent / ".cargo/config", parent / ".cargo/config.toml") if p.is_file())
    cargo_home = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo"))
    configs.update(p for p in (cargo_home / "config", cargo_home / "config.toml") if p.is_file())
    environment = {k: v for k, v in os.environ.items()
                   if k.startswith(("RUST", "CARGO_", "CC", "CXX", "AR", "LINK"))}
    return {"rustc": subprocess.check_output(["rustc", "-Vv"], cwd=repo, text=True).strip(),
            "cargo": subprocess.check_output(["cargo", "-V"], cwd=repo, text=True).strip(),
            "config_sha256": {p.as_posix(): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(configs)},
            "environment_sha256": hashlib.sha256(json.dumps(environment, sort_keys=True).encode()).hexdigest()}


def verify_frozen(source, proof):
    actual = {p.relative_to(source).as_posix() for p in source.rglob("*")
              if p.is_file() and p != source / "source-proof.json"}
    if actual != set(proof["files"]):
        raise RuntimeError(f"Cached historical source file set was modified: {source}")
    for name, expected in proof["files"].items():
        if hashlib.sha256((source / name).read_bytes()).hexdigest() != expected:
            raise RuntimeError(f"Cached historical source was modified: {source / name}")


def available(repo, revision):
    """Whether this clone holds the commit (private anchors are absent from public clones)."""
    try:
        git(repo, "cat-file", "-e", f"{revision}^{{commit}}")
        return True
    except subprocess.CalledProcessError:
        return False


def snapshot(repo, cache, revision, name):
    """Verified git source, with only package metadata/path dependencies adapted."""
    if not available(repo, revision):
        raise RuntimeError(f"Historical policy {name} needs commit {revision}, which this clone lacks")
    core_paths = [f"crates/{p}" for p in SHARED]
    compatible = next((anchor for anchor in COMPATIBLE_CORES
                       if available(repo, anchor) and not git(repo, "diff", anchor, "--", *core_paths)), None)
    untracked = git(repo, "ls-files", "--others", "--exclude-standard", "--",
                    *[f"crates/{p}" for p in SHARED]).decode()
    if compatible is None or untracked:
        raise RuntimeError("The shared engine/agent/knowledge differs from the reviewed historical adapters. "
                           "Review and pin core compatibility before running historical policies.")
    # Separate from earlier caches which shared the current solver. Never silently
    # change the baseline's deadline work by substituting a faster current solver.
    destination = cache / f"policy-{name}-{revision[:12]}-frozen-solver-v2"
    manifest = destination / "source-proof.json"
    if manifest.exists():
        proof = json.loads(manifest.read_text())
        verify_frozen(destination, proof)
        # Paths in adapted Cargo files are specific to this checkout; verify below every time.
    else:
        destination.mkdir(parents=True, exist_ok=True)
        archive = git(repo, "archive", revision, *[f"crates/{p}" for p in FROZEN])
        with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
            for entry in tar:
                if not entry.isfile():
                    continue
                relative = Path(entry.name)
                target = (destination / relative).resolve()
                if not target.is_relative_to(destination.resolve()):
                    raise RuntimeError("Invalid source archive path")
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(tar.extractfile(entry).read())
    workspace = tomllib.loads(git(repo, "show", f"{revision}:Cargo.toml").decode())["workspace"]["package"]
    for package in FROZEN:
        text = git(repo, "show", f"{revision}:crates/{package}/Cargo.toml").decode()
        text = text.replace(f'name = "marjapussi-{package}"', f'name = "marjapussi-{package}-{name}"', 1)
        for key, value in workspace.items():
            text = text.replace(f"{key}.workspace = true", f"{key} = {json.dumps(value)}")
        for dependency in SHARED:
            text = text.replace(f'path = "../{dependency}"',
                                f'path = {json.dumps((repo / "crates" / dependency).as_posix())}')
        for dependency in FROZEN:
            text = text.replace(f'package = "marjapussi-{dependency}"', f'package = "marjapussi-{dependency}-{name}"')
        cargo_path = destination / "crates" / package / "Cargo.toml"
        if not cargo_path.exists() or cargo_path.read_text(encoding="utf-8") != text:
            cargo_path.write_text(text, encoding="utf-8")
    files = {p.relative_to(destination).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
             for p in destination.rglob("*") if p.is_file() and p != manifest}
    manifest.write_text(json.dumps({"revision": revision, "compatible_core": compatible,
                                   "frozen_crates": list(FROZEN), "files": files}, indent=2))
    return destination


def without_deadlines(params_source):
    """Rust statements removing a frozen policy's wall-clock limits under `--deadlines off`.

    Policies with `Params::deadlines` switch them off; older ones only have the per-phase
    budgets, set beyond reach (their endgame search keeps its own 45 ms wall limit).
    """
    path = Path(params_source)
    text = path.read_text(encoding="utf-8") if path.exists() else ""
    if "deadlines: bool" in text:
        return "p.deadlines = setup.deadlines;"
    budgets = [b for b in ("planning_budget_ms", "bidding_budget_ms", "play_budget_ms") if f"{b}: u32" in text]
    return ("if !setup.deadlines { " + " ".join(f"p.{b} = u32::MAX;" for b in budgets) + " }") if budgets else ""


REVISION_PREFIX = "rev:"


def frozen_policies(repo, requested):
    """Frozen policies by name: the named HISTORICAL revisions and `rev:<commit>` lineups.

    Returns name -> (crate alias, full revision). A `rev:` policy freezes any local commit's
    heuristic, beliefs and solver, so candidates are compared without touching the checkout.
    """
    frozen = {}
    for name in requested:
        if name in HISTORICAL:
            frozen[name] = (name, HISTORICAL[name])
        elif name.startswith(REVISION_PREFIX):
            revision = git(repo, "rev-parse", "--verify", name[len(REVISION_PREFIX):] + "^{commit}").decode().strip()
            frozen[name] = ("r" + revision[:12], revision)
    return frozen


def prepare(repo, cache, requested, log_path):
    repo, cache = Path(repo).resolve(), Path(cache).resolve()
    cache.mkdir(parents=True, exist_ok=True)
    # The CLI's outer lock owns machine resources. This lock owns this cache,
    # including callers that selected a different outer lock.
    with RunLock(cache / CACHE_LOCK):
        return _prepare_locked(repo, cache, requested, log_path)


def _prepare_locked(repo, cache, requested, log_path):
    digest = fingerprint(repo)
    compiler = toolchain(repo)
    frozen = frozen_policies(repo, requested)
    historical_sources = {name: snapshot(repo, cache, revision, alias) for name, (alias, revision) in frozen.items()}
    aliases = {name: alias for name, (alias, _) in frozen.items()}
    builder = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    versions = {name: revision for name, (_, revision) in frozen.items()}
    proofs = {name: json.loads((source / "source-proof.json").read_text())
              for name, source in historical_sources.items()}
    key = hashlib.sha256(f"worker-v1:{digest}:{builder}:{versions}:{json.dumps(compiler, sort_keys=True)}".encode()).hexdigest()[:20]
    binary = cache / "bin" / key / WORKER_BINARY
    record = binary.parent / "build.json"
    metadata = {"revision": git(repo, "rev-parse", "HEAD").decode().strip(),
                "source_sha256": digest, "historical_policies": versions,
                "historical_source_proofs": proofs,
                "build_key": key, "builder_sha256": builder, "profile": "release opt3 thin-LTO codegen-units1",
                "working_changes": git(repo, "status", "--porcelain").decode().splitlines(), "toolchain": compiler}
    # Only the last, atomically published record marks a completed entry. A
    # previous interrupted copy without that record must resolve/clean/build again.
    if record.exists():
        saved = json.loads(record.read_text())
        for field in ("build_key", "source_sha256", "builder_sha256", "historical_policies", "toolchain"):
            if saved[field] != metadata[field]:
                raise RuntimeError(f"Cached worker provenance differs: {field}")
        actual = hashlib.sha256(binary.read_bytes()).hexdigest()
        if actual != saved["binary_sha256"]:
            raise RuntimeError(f"Cached worker was modified: {binary}")
        if hashlib.sha256((binary.parent / "Cargo.lock").read_bytes()).hexdigest() != saved["lock_sha256"]:
            raise RuntimeError(f"Cached dependency lock was modified: {binary.parent}")
        for name, expected in saved["harness_files"].items():
            if hashlib.sha256((binary.parent / name).read_bytes()).hexdigest() != expected:
                raise RuntimeError(f"Cached harness source was modified: {binary.parent / name}")
        metadata.update(binary_sha256=actual, lock_sha256=saved["lock_sha256"],
                        harness_files=saved["harness_files"], build_provenance=saved["build_provenance"])
        return binary, metadata
    harness = cache / "harness"
    (harness / "src").mkdir(parents=True, exist_ok=True)
    dependencies = "\n".join(
        f'{name} = {{ package = "marjapussi-{name}", path = {json.dumps((repo / "crates" / name).as_posix())} }}'
        for name in ("tools", "agent", "heuristic"))
    # Several names of one commit (`rev:abc`, `rev:abcdef`) share its frozen crate.
    for alias, source in {aliases[name]: source for name, source in historical_sources.items()}.items():
        dependencies += (f'\n{alias} = {{ package = "marjapussi-heuristic-{alias}", '
                         f'path = {json.dumps((source / "crates/heuristic").as_posix())} }}')
    cargo = f'''[package]
name = "marja-selfplay"
version = "0.1.0"
edition = "2021"
[workspace]
[dependencies]
{dependencies}
[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
strip = "debuginfo"
panic = "abort"
'''
    (harness / "Cargo.toml").write_text(cargo, encoding="utf-8")
    if not (harness / "Cargo.lock").exists() and (repo / "Cargo.lock").exists():
        # Offline resolution starts from the workspace's locked (and locally cached) versions.
        shutil.copy2(repo / "Cargo.lock", harness / "Cargo.lock")
    historical = "".join(f'''
    policies.push(Policy {{
        name: {json.dumps(name)}.into(), parameters: format!("{{:?}}; effective seed supplied by worker; see policy_seed", {alias}::Params::default()),
        make: Box::new(|setup: Setup| {{
            let mut p = {alias}::Params {{ seed: setup.seed, ..{alias}::Params::default() }};
            {without_deadlines(source / "crates/heuristic/src/params.rs")}
            Box::new({alias}::HeuristicAgent::new(p))
        }}),
    }});''' for name, source in historical_sources.items() for alias in [aliases[name]])
    source = '''use tools::selfplay::{builtin_policies, run_with, Policy, Setup};
fn main() {
    let mut policies = builtin_policies();
    policies.push(Policy {
        name: "new".into(), parameters: format!("{:?}; effective seed supplied by worker; see policy_seed", heuristic::Params::default()),
        make: Box::new(|setup: Setup| Box::new(heuristic::HeuristicAgent::new(heuristic::Params {
            seed: setup.seed, deadlines: setup.deadlines, ..heuristic::Params::default() }))),
    });
''' + historical + '''
    if let Err(error) = run_with(policies) { eprintln!("{error}"); std::process::exit(1); }
}
'''
    (harness / "src/main.rs").write_text(source, encoding="utf-8")
    target = cache / "target"
    env = dict(os.environ, CARGO_BUILD_JOBS="1", CARGO_TARGET_DIR=str(target))
    manifest = harness / "Cargo.toml"
    common = ["--offline", "--manifest-path", str(manifest)]
    resolve_command = ["cargo", "metadata", "--format-version=1", *common]
    # Resolve first, then freeze that lock for package-scoped clean and build.
    # A new final-cache key alone does not invalidate Cargo's mtime-based units.
    with Path(log_path).open("wb") as log:
        resolved = subprocess.run(resolve_command, cwd=repo, env=env, stdout=subprocess.PIPE,
                                  stderr=log, check=True)
        graph = json.loads(resolved.stdout)
        active = {node["id"] for node in graph["resolve"]["nodes"]}
        local = {p["id"]: p for p in graph["packages"] if p["id"] in active and p["source"] is None}
        workers = [p for p in local.values() if Path(p["manifest_path"]).resolve() == manifest]
        if len(workers) != 1 or workers[0]["name"] != "marja-selfplay":
            raise RuntimeError("Resolved graph has no unique benchmark worker")
        worker_id = workers[0]["id"]
        packages = sorted(local)
        clean_command = ["cargo", "clean", *common, "--locked", "--release",
                         *[arg for package in packages for arg in ("-p", package)]]
        build_command = ["cargo", "build", *common, "--locked", "--release", "--message-format=json"]
        inputs = {name: hashlib.sha256((harness / name).read_bytes()).hexdigest()
                  for name in ("Cargo.toml", "Cargo.lock", "src/main.rs")}
        subprocess.run(clean_command, cwd=repo, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
        subprocess.run(build_command,
                       cwd=repo, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
    executable, artifacts = completed_artifacts(log_path, local, worker_id, harness, target)
    if fingerprint(repo) != digest or toolchain(repo) != compiler:
        raise RuntimeError("Source or build configuration changed during compilation; no reusable binary was published.")
    for name, expected in inputs.items():
        if hashlib.sha256((harness / name).read_bytes()).hexdigest() != expected:
            raise RuntimeError(f"Harness input changed during compilation: {name}")
    for name, source in historical_sources.items():
        verify_frozen(source, proofs[name])
    binary.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(executable, binary)
    shutil.copy2(harness / "Cargo.lock", binary.parent / "Cargo.lock")
    for source, destination in (("src/main.rs", "harness-main.rs"), ("Cargo.toml", "harness-Cargo.toml")):
        shutil.copy2(harness / source, binary.parent / destination)
    metadata["harness_files"] = {name: hashlib.sha256((binary.parent / name).read_bytes()).hexdigest()
                                 for name in ("harness-main.rs", "harness-Cargo.toml")}
    metadata["binary_sha256"] = hashlib.sha256(binary.read_bytes()).hexdigest()
    metadata["lock_sha256"] = hashlib.sha256((harness / "Cargo.lock").read_bytes()).hexdigest()
    metadata["build_provenance"] = {
        "target": str(target), "resolve_command": resolve_command, "clean_command": clean_command,
        "build_command": build_command, "invalidated_packages": packages,
        "compiler_artifacts": artifacts,
        "build_log_sha256": hashlib.sha256(Path(log_path).read_bytes()).hexdigest(),
    }
    temporary = record.with_suffix(".json.tmp")
    temporary.write_text(json.dumps(metadata, indent=2), encoding="utf-8")
    os.replace(temporary, record)
    return binary, metadata


def completed_artifacts(log_path, local, worker_id, harness, target):
    """Reject mtime reuse after explicit invalidation; bind publication to Cargo's worker event."""
    artifacts, seen, workers = [], set(), []
    finished = False
    for line in Path(log_path).read_text(encoding="utf-8", errors="replace").splitlines():
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue  # Cargo's human-readable progress and compiler diagnostics.
        if not isinstance(event, dict):
            continue
        if event.get("reason") == "build-finished":
            finished = event.get("success") is True
        if event.get("reason") != "compiler-artifact" or event.get("package_id") not in local:
            continue
        package = event["package_id"]
        if event.get("fresh") is not False:
            raise RuntimeError(f"Invalidated local package was reused: {package}")
        if Path(event["manifest_path"]).resolve() != Path(local[package]["manifest_path"]).resolve():
            raise RuntimeError(f"Compiler artifact manifest differs: {package}")
        seen.add(package)
        artifacts.append(event)
        if package == worker_id and event["target"]["kind"] == ["bin"]:
            workers.append(event)
    if not finished or seen != set(local):
        raise RuntimeError(f"Incomplete local compilation evidence; missing: {sorted(set(local) - seen)}")
    if len(workers) != 1:
        raise RuntimeError("Expected one compiled benchmark worker artifact")
    worker = workers[0]
    if (worker["target"]["name"] != "marja-selfplay"
            or worker["target"]["crate_types"] != ["bin"]
            or Path(worker["target"]["src_path"]).resolve() != harness / "src/main.rs"
            or worker["profile"]["opt_level"] != "3" or worker["profile"]["test"] is not False
            or not worker.get("executable")):
        raise RuntimeError("Unexpected benchmark worker target/profile")
    executable = Path(worker["executable"]).resolve()
    if (not executable.is_relative_to(target) or not executable.is_file()
            or executable not in [Path(p).resolve() for p in worker["filenames"]]):
        raise RuntimeError("Benchmark executable is outside the target or absent from its artifact")
    return executable, artifacts
