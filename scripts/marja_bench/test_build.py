"""Historical adapter isolation checks; synthetic Git archive, no Cargo or workers."""

import hashlib
import io
import json
from pathlib import Path
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
        if args[0] in ("diff", "ls-files"):
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


if __name__ == "__main__":
    unittest.main()
