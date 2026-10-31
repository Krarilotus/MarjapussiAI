"""Runner lifecycle tests; no Cargo, real workers, or shared validation lock."""

import importlib.util
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
from types import ModuleType, SimpleNamespace
import unittest
from unittest.mock import MagicMock, Mock, patch


def load_runner():
    dependencies = {}
    for name, attributes in {
        "build": {"CACHE_LOCK": ".build.lock", "POLICIES": {}, "REVISION_PREFIX": "rev:", "prepare": Mock()},
        "report": {"compact_game": lambda row: row, "write_reports": Mock()},
        "resources": {"Resources": Mock(), "RunLock": Mock()},
    }.items():
        module = ModuleType(name)
        module.__dict__.update(attributes)
        dependencies[name] = module
    spec = importlib.util.spec_from_file_location("runner_under_test", Path(__file__).with_name("__main__.py"))
    module = importlib.util.module_from_spec(spec)
    with patch.dict(sys.modules, dependencies):
        spec.loader.exec_module(module)
    return module


runner = load_runner()
ARTIFACTS = ("report.md", "report.html", "games.csv", "summary.json")


class RunnerTests(unittest.TestCase):
    def test_cache_lock_cannot_also_be_the_outer_resource_lock(self):
        with tempfile.TemporaryDirectory() as directory:
            cache = Path(directory) / "cache"
            argv = ["marja-bench", "hp1", "hp1", "hp1", "hp1", "2",
                    "--cache", str(cache), "--lock", str(cache / ".build.lock")]
            with patch.object(sys, "argv", argv), patch.object(runner, "RunLock") as lock, \
                    patch.object(runner, "prepare") as prepare:
                with self.assertRaisesRegex(RuntimeError, "resource lock must differ"):
                    runner.main()
                lock.assert_not_called()
                prepare.assert_not_called()

    def setUp(self):
        scratch = Path(__file__).resolve().parents[2] / "target/runner-tests"
        scratch.mkdir(parents=True, exist_ok=True)
        temporary = tempfile.TemporaryDirectory(prefix="case-", dir=scratch)
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name)
        self.manifest = {"status": "complete", "games": 1, "writers_stopped": True}
        self.options = SimpleNamespace(workers=1, games=1, seed=1, rotation="teams-and-seats", policy_seed=None, compact=False, deadlines="wall",
                                       policies=["new", "old", "new", "old"])
        self.resources = Mock()
        self.resources.describe.return_value = {}

    def write_reports(self, directory, rows, manifest):
        result = {"complete": manifest["status"] == "complete", "count": len(rows)}
        for name in ARTIFACTS:
            (directory / name).write_text(json.dumps(result), encoding="utf-8")
        return result

    def stored_manifest(self):
        return json.loads((self.directory / "manifest.json").read_text(encoding="utf-8"))

    def worker(self, *, writer_alive=False, exit_code=0):
        process = Mock()
        process.stdout = io.StringIO(json.dumps({"schema": "marjapussi.selfplay/1", "index": 0}) + "\n")
        process.poll.return_value = exit_code
        process.wait.return_value = exit_code
        thread = Mock()
        thread.is_alive.return_value = writer_alive

        def make_thread(**kwargs):
            thread.start.side_effect = lambda: kwargs["target"](*kwargs["args"])
            return thread

        return process, thread, make_thread

    def run_worker(self, process, make_thread):
        with patch.object(runner.subprocess, "CREATE_NO_WINDOW", 0, create=True), \
                patch.object(runner.subprocess, "Popen", return_value=process), \
                patch.object(runner.threading, "Thread", side_effect=make_thread), \
                patch.object(runner, "write_reports", side_effect=self.write_reports):
            return runner.run_workers(Path("unused-worker"), self.options, self.resources,
                                      self.directory, self.manifest)

    def test_reread_only_after_writer_join_and_liveness_check(self):
        process, thread, make_thread = self.worker()
        load_games = runner.load_games

        def checked_load(directory):
            thread.join.assert_called_once()
            thread.is_alive.assert_called_once()
            self.assertTrue(self.manifest["writers_stopped"])
            self.assertEqual(self.stored_manifest()["status"], "reporting")
            return load_games(directory)

        with patch.object(runner, "load_games", side_effect=checked_load):
            self.run_worker(process, make_thread)
        self.assertEqual(self.stored_manifest()["status"], "complete")

    def test_team_job_is_128_blocks_and_forwards_zero_without_dropping_it(self):
        options = SimpleNamespace(workers=2, games=256, seed=20261013, rotation="teams", policy_seed=0, compact=False, deadlines="wall",
                                  policies=["new", "previous", "new", "previous"])
        metadata = runner.rotation_metadata(options.games, options.rotation)
        self.assertEqual((metadata["distinct_deals"], metadata["full_rotation_blocks"],
                          metadata["games_per_deal"], metadata["partial_block_games"]), (128, 128, 2, 0))
        command = runner.worker_command(Path("worker.exe"), options, 1)
        self.assertEqual(command[command.index("--rotation") + 1], "teams")
        self.assertEqual(command[command.index("--policy-seed") + 1], "0")
        self.assertEqual(command[command.index("--worker-index") + 1], "1")
        for seat, policy in enumerate(options.policies):
            self.assertEqual(command[command.index(f"--seat{seat}") + 1], policy)
        options.policy_seed = None
        self.assertNotIn("--policy-seed", runner.worker_command(Path("worker.exe"), options, 0))
        self.assertEqual(runner.rotation_metadata(256, "teams-and-seats")["distinct_deals"], 64)

    def test_stuck_writer_skips_raw_reread_and_report(self):
        process, thread, make_thread = self.worker(writer_alive=True)
        with patch.object(runner, "load_games") as load:
            with self.assertRaisesRegex(RuntimeError, "writer did not stop"):
                self.run_worker(process, make_thread)
            load.assert_not_called()
        manifest = self.stored_manifest()
        self.assertFalse(manifest["writers_stopped"])
        self.assertEqual(manifest["status"], "interrupted_or_failed")
        self.assertEqual(manifest["report_status"], "skipped_writers_active")

    def test_kill_wait_is_bounded_and_all_readers_are_checked(self):
        process, thread = Mock(), Mock()
        process.poll.return_value = None
        process.wait.side_effect = subprocess.TimeoutExpired("worker", 5)
        thread.is_alive.return_value = False
        with self.assertRaisesRegex(RuntimeError, "teardown failed"):
            runner.stop_workers([process], [thread])
        process.terminate.assert_called_once()
        process.kill.assert_called_once()
        self.assertEqual([call.kwargs for call in process.wait.call_args_list], [{"timeout": 5}, {"timeout": 5}])
        thread.join.assert_called_once()
        thread.is_alive.assert_called_once()

    def test_nonzero_worker_with_full_raw_sample_remains_failed(self):
        process, thread, make_thread = self.worker(exit_code=3)
        with self.assertRaisesRegex(RuntimeError, "Worker 0 exited 3"):
            self.run_worker(process, make_thread)
        manifest = self.stored_manifest()
        self.assertEqual(manifest["status"], "interrupted_or_failed")
        self.assertFalse(manifest["games_complete"])
        summary = json.loads((self.directory / "summary.json").read_text(encoding="utf-8"))
        self.assertFalse(summary["complete"])

    def test_successful_workers_followed_by_report_failure_do_not_claim_complete(self):
        process, thread, make_thread = self.worker()
        with patch.object(self, "write_reports", side_effect=ValueError("invalid scoring")):
            with self.assertRaisesRegex(ValueError, "invalid scoring"):
                self.run_worker(process, make_thread)
        manifest = self.stored_manifest()
        self.assertTrue(manifest["games_complete"])
        self.assertTrue(manifest["writers_stopped"])
        self.assertEqual(manifest["status"], "report_failed")
        self.assertEqual(manifest["run_status"], "complete")
        self.assertFalse((self.directory / "summary.json").exists())

    def test_report_generation_failure_keeps_old_artifacts_and_truthful_status(self):
        for name in ARTIFACTS:
            (self.directory / name).write_text("old", encoding="utf-8")

        def fail(staging, *_):
            (staging / "report.md").write_text("partial replacement", encoding="utf-8")
            raise ValueError("invalid scoring")

        with patch.object(runner, "load_games", return_value=[{"index": 0}]), \
                patch.object(runner, "write_reports", side_effect=fail):
            with self.assertRaisesRegex(ValueError, "invalid scoring"):
                runner.generate_reports(self.directory, self.manifest, "complete")
        self.assertEqual(self.stored_manifest()["status"], "report_failed")
        self.assertEqual(self.stored_manifest()["run_status"], "complete")
        self.assertTrue(all((self.directory / name).read_text(encoding="utf-8") == "old" for name in ARTIFACTS))
        self.assertFalse(list(self.directory.glob(".report-*")))

    def test_publication_failure_does_not_publish_complete_summary(self):
        (self.directory / "summary.json").write_text("old", encoding="utf-8")
        replace = runner.os.replace

        def fail_csv(source, destination):
            if Path(destination).name == "games.csv":
                raise OSError("publication blocked")
            return replace(source, destination)

        with patch.object(runner, "load_games", return_value=[{"index": 0}]), \
                patch.object(runner, "write_reports", side_effect=self.write_reports), \
                patch.object(runner.os, "replace", side_effect=fail_csv):
            with self.assertRaisesRegex(OSError, "publication blocked"):
                runner.generate_reports(self.directory, self.manifest, "complete")
        self.assertEqual(self.stored_manifest()["status"], "report_failed")
        self.assertEqual((self.directory / "summary.json").read_text(encoding="utf-8"), "old")

    def test_report_only_rejects_active_or_unconfirmed_writer_before_reread(self):
        for changes in ({"status": "running"}, {"status": "reporting"},
                        {"status": "prepared"}, {"writers_stopped": False}):
            with self.subTest(changes=changes):
                runner.save_manifest(self.directory, dict(self.manifest, **changes))
                with patch.object(runner, "RunLock") as lock, patch.object(runner, "load_games") as load:
                    with self.assertRaises(RuntimeError):
                        runner.report_existing(self.directory, Path("unused-global-lock"))
                    self.assertEqual([call.args[0] for call in lock.call_args_list],
                                     [Path("unused-global-lock"), self.directory / ".run.lock"])
                    load.assert_not_called()

    def test_report_only_lock_failure_prevents_reread(self):
        with patch.object(runner, "RunLock") as lock, patch.object(runner, "load_games") as load:
            lock.return_value.__enter__.side_effect = RuntimeError("busy")
            with self.assertRaisesRegex(RuntimeError, "busy"):
                runner.report_existing(self.directory, Path("unused-global-lock"))
            load.assert_not_called()

    def test_per_run_lock_failure_prevents_reread_even_with_different_global_lock(self):
        runner.save_manifest(self.directory, self.manifest)
        with patch.object(runner, "RunLock") as lock, patch.object(runner, "load_games") as load:
            global_lock, run_lock = MagicMock(), MagicMock()
            run_lock.__enter__.side_effect = RuntimeError("run busy")
            lock.side_effect = [global_lock, run_lock]
            with self.assertRaisesRegex(RuntimeError, "run busy"):
                runner.report_existing(self.directory, Path("different-global-lock"))
            load.assert_not_called()
            global_lock.__exit__.assert_called_once()

    def test_report_only_can_recover_report_failure_without_changing_run_outcome(self):
        for outcome in ("complete", "interrupted_or_failed"):
            with self.subTest(outcome=outcome):
                manifest = dict(self.manifest, status="report_failed", run_status=outcome, report_error="old error")
                runner.save_manifest(self.directory, manifest)
                with patch.object(runner, "RunLock"), \
                        patch.object(runner, "load_games", return_value=[{"index": 0}]), \
                        patch.object(runner, "write_reports", side_effect=self.write_reports):
                    runner.report_existing(self.directory, Path("unused-global-lock"))
                stored = self.stored_manifest()
                self.assertEqual(stored["status"], outcome)
                self.assertEqual(stored["report_status"], "complete")
                self.assertNotIn("report_error", stored)


if __name__ == "__main__":
    unittest.main()
