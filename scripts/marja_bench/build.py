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

OLD = "1632af833199e5253271d493af8531badfac81c0"
COMPATIBLE_CORE = "b3abdaf1f5e47111904db442ce36db7dc4ae9e0b"
HISTORICAL = {"old": OLD, "previous": COMPATIBLE_CORE}
# Source-reviewed additive knowledge helpers; existing core algorithms unchanged.
# Keep this exact anchor so any subsequent core change requires another review.
COMPATIBLE_CORES = (
    COMPATIBLE_CORE,
    "bcf567164d142c969ace2ed63b14e0e6ddedde2e",
    "4bff913bead8f33bcdce27751cfa0cb6e2296e71",
)
SHARED = ("engine", "observation", "knowledge", "agent")
FROZEN = ("heuristic", "beliefs", "solver")
POLICIES = {
    "new": "Current HP1, full beliefs and continuation planning (alias: hp1)",
    "old": f"Exact previous heuristic and beliefs, {OLD[:12]}",
    "previous": f"Verified live HP1 with frozen beliefs and solver, {COMPATIBLE_CORE[:12]}",
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


def snapshot(repo, cache, revision, name):
    """Verified git source, with only package metadata/path dependencies adapted."""
    core_paths = [f"crates/{p}" for p in SHARED]
    compatible = next((revision for revision in COMPATIBLE_CORES
                       if not git(repo, "diff", revision, "--", *core_paths)), None)
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


def prepare(repo, cache, requested, log_path):
    repo, cache = Path(repo).resolve(), Path(cache).resolve()
    cache.mkdir(parents=True, exist_ok=True)
    digest = fingerprint(repo)
    compiler = toolchain(repo)
    historical_sources = {name: snapshot(repo, cache, revision, name) for name, revision in HISTORICAL.items() if name in requested}
    builder = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    versions = {name: HISTORICAL[name] for name in historical_sources}
    proofs = {name: json.loads((source / "source-proof.json").read_text())
              for name, source in historical_sources.items()}
    key = hashlib.sha256(f"worker-v1:{digest}:{builder}:{versions}:{json.dumps(compiler, sort_keys=True)}".encode()).hexdigest()[:20]
    binary = cache / "bin" / key / "marja-selfplay.exe"
    metadata = {"revision": git(repo, "rev-parse", "HEAD").decode().strip(),
                "source_sha256": digest, "historical_policies": versions,
                "historical_source_proofs": proofs,
                "build_key": key, "builder_sha256": builder, "profile": "release opt3 thin-LTO codegen-units1",
                "working_changes": git(repo, "status", "--porcelain").decode().splitlines(), "toolchain": compiler}
    if binary.exists():
        saved = json.loads((binary.parent / "build.json").read_text())
        actual = hashlib.sha256(binary.read_bytes()).hexdigest()
        if actual != saved["binary_sha256"]:
            raise RuntimeError(f"Cached worker was modified: {binary}")
        if hashlib.sha256((binary.parent / "Cargo.lock").read_bytes()).hexdigest() != saved["lock_sha256"]:
            raise RuntimeError(f"Cached dependency lock was modified: {binary.parent}")
        for name, expected in saved["harness_files"].items():
            if hashlib.sha256((binary.parent / name).read_bytes()).hexdigest() != expected:
                raise RuntimeError(f"Cached harness source was modified: {binary.parent / name}")
        metadata.update(binary_sha256=actual, lock_sha256=saved["lock_sha256"], harness_files=saved["harness_files"])
        return binary, metadata
    harness = cache / "harness"
    (harness / "src").mkdir(parents=True, exist_ok=True)
    dependencies = "\n".join(
        f'{name} = {{ package = "marjapussi-{name}", path = {json.dumps((repo / "crates" / name).as_posix())} }}'
        for name in ("tools", "agent", "heuristic"))
    for name, source in historical_sources.items():
        dependencies += (f'\n{name} = {{ package = "marjapussi-heuristic-{name}", '
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
    historical = "".join(f'''
    policies.push(Policy {{
        name: "{name}".into(), parameters: format!("{{:?}}; effective seed supplied by worker; see policy_seed", {name}::Params::default()),
        make: Box::new(|seed, _| Box::new({name}::HeuristicAgent::new({name}::Params {{ seed, ..{name}::Params::default() }}))),
    }});''' for name in historical_sources)
    source = '''use tools::selfplay::{Policy, AssessmentRecorder, builtin_policies_with_recording, record_current, run_with_recording};
fn main() {
    let recorder = AssessmentRecorder::default();
    let mut policies = builtin_policies_with_recording(recorder.clone());
    let current_recorder = recorder.clone();
    policies.push(Policy {
        name: "new".into(), parameters: format!("{:?}; effective seed supplied by worker; see policy_seed", heuristic::Params::default()),
        make: Box::new(move |seed, _| record_current(heuristic::HeuristicAgent::new(heuristic::Params { seed, ..heuristic::Params::default() }), current_recorder.clone())),
    });
''' + historical + '''
    if let Err(error) = run_with_recording(policies, recorder) { eprintln!("{error}"); std::process::exit(1); }
}
'''
    (harness / "src/main.rs").write_text(source, encoding="utf-8")
    env = dict(os.environ, CARGO_BUILD_JOBS="1", CARGO_TARGET_DIR=str(cache / "target"))
    # Resolve only from already available dependencies; retain the resulting lock for this build.
    with Path(log_path).open("wb") as log:
        subprocess.run(["cargo", "build", "--offline", "--release", "--manifest-path", str(harness / "Cargo.toml")],
                       cwd=repo, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
    if fingerprint(repo) != digest or toolchain(repo) != compiler:
        raise RuntimeError("Source or build configuration changed during compilation; no reusable binary was published.")
    for name, source in historical_sources.items():
        verify_frozen(source, proofs[name])
    binary.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(cache / "target/release/marja-selfplay.exe", binary)
    shutil.copy2(harness / "Cargo.lock", binary.parent / "Cargo.lock")
    for source, target in (("src/main.rs", "harness-main.rs"), ("Cargo.toml", "harness-Cargo.toml")):
        shutil.copy2(harness / source, binary.parent / target)
    metadata["harness_files"] = {name: hashlib.sha256((binary.parent / name).read_bytes()).hexdigest()
                                 for name in ("harness-main.rs", "harness-Cargo.toml")}
    metadata["binary_sha256"] = hashlib.sha256(binary.read_bytes()).hexdigest()
    metadata["lock_sha256"] = hashlib.sha256((harness / "Cargo.lock").read_bytes()).hexdigest()
    (binary.parent / "build.json").write_text(json.dumps(metadata, indent=2), encoding="utf-8")
    return binary, metadata
