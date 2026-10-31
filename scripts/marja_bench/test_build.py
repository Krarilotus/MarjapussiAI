"""Build provenance and historical isolation; mocked Cargo, no compiler or workers."""

import hashlib
import io
import json
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch

import build


class FrozenSolverTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.repo = Path(self.temporary.name) / "repo"
        self.repo.mkdir()
        self.cache = Path(self.temporary.name) / "cache"
        self.calls = []
        self.cargos = {}
        self.sources = {}
        for crate in build.FROZEN:
            dependencies = ('beliefs = { package = "marjapussi-beliefs", path = "../beliefs" }\n'
                            'solver = { package = "marjapussi-solver", path = "../solver" }\n') if crate == "heuristic" else ""
            self.cargos[crate] = (f'[package]\nname = "marjapussi-{crate}"\nedition.workspace = true\n'
                                  '[dependencies]\nknowledge = { package = "marjapussi-knowledge", path = "../knowledge" }\n'
                                  + dependencies).encode()
            self.sources[f"crates/{crate}/Cargo.toml"] = self.cargos[crate]
            self.sources[f"crates/{crate}/src/lib.rs"] = f"// frozen {crate} implementation\n".encode()
        archive = io.BytesIO()
        with tarfile.open(fileobj=archive, mode="w") as tar:
            for name, data in self.sources.items():
                entry = tarfile.TarInfo(name)
                entry.size = len(data)
                tar.addfile(entry, io.BytesIO(data))
        self.archive = archive.getvalue()

    def git(self, repo, *args):
        self.calls.append(args)
        if args[0] in ("diff", "ls-files", "cat-file"):
            return b""
        if args[0] == "archive":
            self.assertEqual(set(args[2:]), {f"crates/{p}" for p in build.FROZEN})
            return self.archive
        if args == ("show", f"{build.COMPATIBLE_CORE}:Cargo.toml"):
            return b'[workspace.package]\nedition = "2021"\n'
        if args[0] == "show":
            return self.cargos[args[1].split("/")[1]]
        raise AssertionError(args)

    def test_solver_source_is_frozen_and_heuristic_dependency_uses_isolated_package(self):
        current = self.repo / "crates/solver/src"
        current.mkdir(parents=True)
        (current / "lib.rs").write_text("// different current algorithm\n")
        with patch.object(build, "git", side_effect=self.git):
            source = build.snapshot(self.repo, self.cache, build.COMPATIBLE_CORE, "previous")
        self.assertEqual((source / "crates/solver/src/lib.rs").read_bytes(), self.sources["crates/solver/src/lib.rs"])
        cargo = (source / "crates/heuristic/Cargo.toml").read_text()
        self.assertIn('package = "marjapussi-solver-previous", path = "../solver"', cargo)
        self.assertIn('name = "marjapussi-solver-previous"', (source / "crates/solver/Cargo.toml").read_text())
        proof = json.loads((source / "source-proof.json").read_text())
        self.assertEqual(proof["frozen_crates"], list(build.FROZEN))
        self.assertEqual(proof["files"]["crates/solver/src/lib.rs"], hashlib.sha256(self.sources["crates/solver/src/lib.rs"]).hexdigest())
        for args in self.calls:
            if args[0] == "diff":
                self.assertNotIn("crates/solver", args)
                self.assertIn("crates/engine", args)
        (source / "crates/solver/src/lib.rs").write_text("// tampered baseline\n")
        with patch.object(build, "git", side_effect=self.git), self.assertRaisesRegex(RuntimeError, "modified"):
            build.snapshot(self.repo, self.cache, build.COMPATIBLE_CORE, "previous")

    def test_shared_core_guard_still_rejects_changed_engine(self):
        def changed(repo, *args):
            return b"changed engine" if args[0] == "diff" else self.git(repo, *args)
        with patch.object(build, "git", side_effect=changed), self.assertRaisesRegex(RuntimeError, "shared engine"):
            build.snapshot(self.repo, self.cache, build.COMPATIBLE_CORE, "previous")


class FrozenPolicyNameTests(unittest.TestCase):
    def test_revisions_and_historical_names_resolve_to_crate_aliases(self):
        full = "0e441f7" + "0" * 33
        def git(repo, *args):
            self.assertEqual(args[:2], ("rev-parse", "--verify"))
            return (full + "\n").encode()
        with patch.object(build, "git", side_effect=git):
            frozen = build.frozen_policies(Path("."), ["new", "rev:0e441f7", "base", "rev:0e441f7" + "0" * 3])
        self.assertEqual(frozen["base"], ("base", build.BASE))
        self.assertEqual(frozen["rev:0e441f7"], ("r" + full[:12], full))
        # Two names of one commit share its crate alias.
        self.assertEqual(frozen["rev:0e441f7000"][0], frozen["rev:0e441f7"][0])
        self.assertNotIn("new", frozen)


class BuildLifecycleTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.repo, self.cache = self.root / "repo", self.root / "cache"
        self.repo.mkdir()
        self.log = self.root / "build.log"
        self.held = False
        self.steps, self.calls = [], []
        self.mode = "ok"
        self.copy = shutil.copy2
        owner = self

        class Lock:
            def __init__(self, path):
                owner.assertEqual(path, owner.cache / build.CACHE_LOCK)

            def __enter__(self):
                owner.assertFalse(owner.held)
                owner.held = True
                owner.steps.append("lock")

            def __exit__(self, *_):
                owner.assertTrue(owner.held)
                owner.steps.append("unlock")
                owner.held = False

        for target, replacement in (
            ("RunLock", Lock), ("fingerprint", self.fingerprint),
            ("toolchain", self.toolchain), ("git", self.git), ("snapshot", self.snapshot),
        ):
            patcher = patch.object(build, target, side_effect=replacement) if target != "RunLock" else patch.object(build, target, replacement)
            patcher.start()
            self.addCleanup(patcher.stop)
        for target, replacement in ((build.subprocess, self.cargo), (build.shutil, self.copy_locked)):
            patcher = patch.object(target, "run" if target is build.subprocess else "copy2", side_effect=replacement)
            patcher.start()
            self.addCleanup(patcher.stop)

    def fingerprint(self, _):
        self.assertTrue(self.held)
        return "changed" if self.mode == "source_changed" and "build" in self.steps else "source"

    def toolchain(self, _):
        self.assertTrue(self.held)
        return {"rustc": "changed" if self.mode == "config_changed" and "build" in self.steps else "fixed"}

    def git(self, _, *args):
        self.assertTrue(self.held)
        return b"revision" if args == ("rev-parse", "HEAD") else b""

    def snapshot(self, repo, cache, revision, name):
        self.assertTrue(self.held)
        self.steps.append("snapshot")
        source = cache / ("policy-" + name)
        source.mkdir(exist_ok=True)
        (source / "source-proof.json").write_text(json.dumps({"files": {}}))
        return source

    def package(self, name, manifest, source=None):
        return {"id": f"path+{manifest.parent.as_uri()}#{name}@0.1.0", "name": name,
                "manifest_path": str(manifest), "source": source}

    def cargo(self, command, **kwargs):
        self.assertTrue(self.held)
        self.calls.append((command, kwargs))
        operation = command[1]
        self.steps.append(operation)
        harness = self.cache / "harness"
        if operation == "metadata":
            (harness / "Cargo.lock").write_text("resolved lock")
            self.local = [self.package("marjapussi-" + name, self.repo / "crates" / name / "Cargo.toml")
                          for name in (*build.SHARED, *build.FROZEN, "eval", "tools")]
            self.local += [self.package("marjapussi-" + name + "-previous",
                                       self.cache / "policy-previous/crates" / name / "Cargo.toml")
                           for name in build.FROZEN]
            self.local.append(self.package("marja-selfplay", harness / "Cargo.toml"))
            registry = self.package("rand", self.root / "registry/Cargo.toml", "registry+example")
            inactive = self.package("unused-local", self.root / "unused/Cargo.toml")
            graph = {"packages": [*self.local, registry, inactive],
                     "resolve": {"nodes": [{"id": p["id"]} for p in [*self.local, registry]]}}
            return subprocess.CompletedProcess(command, 0, stdout=json.dumps(graph).encode())
        if self.mode == operation + "_failed":
            raise subprocess.CalledProcessError(1, command)
        if operation == "clean":
            return subprocess.CompletedProcess(command, 0)
        self.assertEqual(operation, "build")
        executable = self.cache / "target/release/deps/worker-exact.exe"
        executable.parent.mkdir(parents=True, exist_ok=True)
        executable.write_bytes(b"compiled worker")
        # A conventional output path is deliberately different: publish the event's artifact.
        (self.cache / "target/release/marja-selfplay.exe").write_bytes(b"wrong conventional output")
        events = []
        for package in self.local:
            worker = package["name"] == "marja-selfplay"
            event = {"reason": "compiler-artifact", "package_id": package["id"],
                     "manifest_path": package["manifest_path"], "fresh": False,
                     "target": {"kind": ["bin" if worker else "lib"],
                                "crate_types": ["bin" if worker else "lib"],
                                "name": "marja-selfplay" if worker else package["name"],
                                "src_path": str(harness / "src/main.rs")},
                     "profile": {"opt_level": "3", "test": False},
                     "executable": str(executable) if worker else None,
                     "filenames": [str(executable)] if worker else []}
            if package == self.local[0]:
                if self.mode == "dependency_missing":
                    continue
                if self.mode == "dependency_fresh":
                    event["fresh"] = True
                if self.mode == "manifest_wrong":
                    event["manifest_path"] = str(self.root / "wrong/Cargo.toml")
            if worker:
                if self.mode == "worker_missing":
                    continue
                if self.mode == "worker_wrong":
                    event["target"]["src_path"] = str(self.root / "wrong.rs")
                if self.mode == "worker_outside":
                    event["executable"] = str(self.root / "outside.exe")
                if self.mode == "worker_duplicate":
                    events.append(event)
            events.append(event)
        if self.mode != "unfinished":
            events.append({"reason": "build-finished", "success": True})
        kwargs["stdout"].write(("cargo progress\n" + "\n".join(map(json.dumps, events))).encode())
        if self.mode == "harness_changed":
            (harness / "src/main.rs").write_text("changed during build")
        return subprocess.CompletedProcess(command, 0)

    def copy_locked(self, source, destination):
        self.assertTrue(self.held)
        self.assertFalse(list(self.cache.glob("bin/*/build.json")))
        self.steps.append("copy")
        if self.mode == "copy_failed" or (self.mode == "later_copy_failed" and self.steps.count("copy") == 2):
            raise OSError("copy failed")
        return self.copy(source, destination)

    def prepare(self):
        return build.prepare(self.repo, self.cache, ["new", "previous"], self.log)

    def test_scoped_clean_build_and_exact_artifact_publication_are_locked(self):
        binary, metadata = self.prepare()
        self.assertEqual(binary.read_bytes(), b"compiled worker")
        self.assertEqual(self.steps[:5], ["lock", "snapshot", "metadata", "clean", "build"])
        self.assertEqual(self.steps[-1], "unlock")
        provenance = metadata["build_provenance"]
        self.assertEqual(provenance["target"], str(self.cache / "target"))
        self.assertEqual(set(provenance["invalidated_packages"]), {p["id"] for p in self.local})
        self.assertEqual(len(self.local), 13)  # Current nine, three frozen aliases, worker.
        self.assertEqual(len(provenance["compiler_artifacts"]), 13)
        clean = self.calls[1][0]
        self.assertEqual(clean[clean.index("-p"):][::2], ["-p"] * 13)
        self.assertEqual(set(clean[clean.index("-p"):][1::2]), {p["id"] for p in self.local})
        for command, kwargs in self.calls:
            self.assertEqual(kwargs["cwd"], self.repo)
            self.assertEqual(kwargs["env"], self.calls[0][1]["env"])
            self.assertEqual(kwargs["env"]["CARGO_TARGET_DIR"], str(self.cache / "target"))
            self.assertEqual(command[command.index("--manifest-path") + 1], str(self.cache / "harness/Cargo.toml"))
        stored = json.loads((binary.parent / "build.json").read_text())
        self.assertEqual(stored, metadata)
        self.assertEqual(provenance["build_log_sha256"], hashlib.sha256(self.log.read_bytes()).hexdigest())

    def test_verified_final_hit_skips_resolution_clean_build_and_copy(self):
        binary, first = self.prepare()
        self.calls.clear()
        self.steps.clear()
        again, metadata = self.prepare()
        self.assertEqual(again, binary)
        self.assertEqual(metadata, first)
        self.assertEqual(self.calls, [])
        self.assertEqual(self.steps, ["lock", "snapshot", "unlock"])
        binary.write_bytes(b"tampered")
        with self.assertRaisesRegex(RuntimeError, "Cached worker was modified"):
            self.prepare()
        self.assertEqual(self.calls, [])

    def test_failures_never_publish_a_completed_build_record(self):
        for mode in ("clean_failed", "build_failed", "dependency_fresh", "dependency_missing",
                     "manifest_wrong", "worker_missing", "worker_wrong", "worker_outside",
                     "worker_duplicate", "unfinished", "source_changed", "config_changed",
                     "harness_changed", "copy_failed", "later_copy_failed"):
            with self.subTest(mode=mode):
                self.cache = self.root / mode
                self.steps.clear()
                self.calls.clear()
                self.mode = mode
                with self.assertRaises((RuntimeError, OSError, subprocess.CalledProcessError)):
                    self.prepare()
                self.assertFalse(self.held)
                self.assertFalse(list(self.cache.glob("bin/*/build.json")))
                if mode == "clean_failed":
                    self.assertNotIn("build", self.steps)

    def test_interrupted_publication_is_rebuilt_before_becoming_a_hit(self):
        self.mode = "later_copy_failed"
        with self.assertRaisesRegex(OSError, "copy failed"):
            self.prepare()
        self.assertEqual(len(list(self.cache.glob(f"bin/*/{build.WORKER_BINARY}"))), 1)
        self.assertFalse(list(self.cache.glob("bin/*/build.json")))
        self.mode = "ok"
        self.steps.clear()
        self.calls.clear()
        with patch.object(build.os, "replace", side_effect=OSError("publication interrupted")):
            with self.assertRaisesRegex(OSError, "publication interrupted"):
                self.prepare()
        self.assertEqual([command[1] for command, _ in self.calls], ["metadata", "clean", "build"])
        self.assertFalse(list(self.cache.glob("bin/*/build.json")))
        self.calls.clear()
        binary, metadata = self.prepare()
        self.assertEqual([command[1] for command, _ in self.calls], ["metadata", "clean", "build"])
        self.assertEqual(json.loads((binary.parent / "build.json").read_text()), metadata)


if __name__ == "__main__":
    unittest.main()
