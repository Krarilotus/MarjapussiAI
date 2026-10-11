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
SHARED = ("engine", "observation", "knowledge", "agent", "solver")
POLICIES = {
    "new": "Current HP1, full beliefs and continuation planning (alias: hp1)",
    "old": f"Exact previous heuristic and beliefs, {OLD[:12]}",
    "previous": f"Full-game rollout policy before conditional concepts, {COMPATIBLE_CORE[:12]}",
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


def snapshot(repo, cache, revision, name):
    """Verified git source, with only package metadata/path dependencies adapted."""
    changed = git(repo, "diff", COMPATIBLE_CORE, "--", *[f"crates/{p}" for p in SHARED]).decode()
    untracked = git(repo, "ls-files", "--others", "--exclude-standard", "--",
                    *[f"crates/{p}" for p in SHARED]).decode()
    if changed or untracked:
        raise RuntimeError("The historical 'old' policy is qualified against the b3abdaf core. "
                           "The shared engine/agent/knowledge/solver has changed; review adapter compatibility first.")
    destination = cache / f"policy-{revision[:12]}"
    manifest = destination / "source-proof.json"
    if manifest.exists():
        proof = json.loads(manifest.read_text())
        for source_name, expected in proof["files"].items():
            if hashlib.sha256((destination / source_name).read_bytes()).hexdigest() != expected:
                raise RuntimeError(f"Cached historical source was modified: {destination / source_name}")
        # Paths in adapted Cargo files are specific to this checkout; verify below every time.
    else:
        destination.mkdir(parents=True, exist_ok=True)
        archive = git(repo, "archive", revision, "crates/heuristic", "crates/beliefs")
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
    for package in ("beliefs", "heuristic"):
        text = git(repo, "show", f"{revision}:crates/{package}/Cargo.toml").decode()
        text = text.replace(f'name = "marjapussi-{package}"', f'name = "marjapussi-{package}-{name}"', 1)
        for key, value in workspace.items():
            text = text.replace(f"{key}.workspace = true", f"{key} = {json.dumps(value)}")
        for dependency in SHARED:
            text = text.replace(f'path = "../{dependency}"',
                                f'path = {json.dumps((repo / "crates" / dependency).as_posix())}')
        if package == "heuristic":
            text = text.replace('package = "marjapussi-beliefs"', f'package = "marjapussi-beliefs-{name}"')
        (destination / "crates" / package / "Cargo.toml").write_text(text, encoding="utf-8")
    files = {p.relative_to(destination).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
             for p in destination.rglob("*") if p.is_file() and p != manifest}
    manifest.write_text(json.dumps({"revision": revision, "compatible_core": COMPATIBLE_CORE, "files": files}, indent=2))
    return destination


def prepare(repo, cache, requested, log_path):
    repo, cache = Path(repo).resolve(), Path(cache).resolve()
    cache.mkdir(parents=True, exist_ok=True)
    digest = fingerprint(repo)
    compiler = toolchain(repo)
    historical_sources = {name: snapshot(repo, cache, revision, name) for name, revision in HISTORICAL.items() if name in requested}
    builder = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    versions = {name: HISTORICAL[name] for name in historical_sources}
    key = hashlib.sha256(f"worker-v1:{digest}:{builder}:{versions}:{json.dumps(compiler, sort_keys=True)}".encode()).hexdigest()[:20]
    binary = cache / "bin" / key / "marja-selfplay.exe"
    metadata = {"revision": git(repo, "rev-parse", "HEAD").decode().strip(),
                "source_sha256": digest, "historical_policies": versions,
                "build_key": key, "builder_sha256": builder, "profile": "release opt3 thin-LTO codegen-units1",
                "working_changes": git(repo, "status", "--porcelain").decode().splitlines(), "toolchain": compiler}
    if binary.exists():
        saved = json.loads((binary.parent / "build.json").read_text())
        actual = hashlib.sha256(binary.read_bytes()).hexdigest()
        if actual != saved["binary_sha256"]:
            raise RuntimeError(f"Cached worker was modified: {binary}")
        if hashlib.sha256((binary.parent / "Cargo.lock").read_bytes()).hexdigest() != saved["lock_sha256"]:
            raise RuntimeError(f"Cached dependency lock was modified: {binary.parent}")
        metadata.update(binary_sha256=actual, lock_sha256=saved["lock_sha256"])
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
        name: "{name}".into(), parameters: format!("{{:?}}; seed derived per deal and slot", {name}::Params::default()),
        make: Box::new(|seed, _| Box::new({name}::HeuristicAgent::new({name}::Params {{ seed, ..{name}::Params::default() }}))),
    }});''' for name in historical_sources)
    source = '''use tools::selfplay::{Policy, builtin_policies, run_with};
fn main() {
    let mut policies = builtin_policies();
    policies.push(Policy {
        name: "new".into(), parameters: format!("{:?}; seed derived per deal and slot", heuristic::Params::default()),
        make: Box::new(|seed, _| Box::new(heuristic::HeuristicAgent::new(heuristic::Params { seed, ..heuristic::Params::default() }))),
    });
''' + historical + '''
    if let Err(error) = run_with(policies) { eprintln!("{error}"); std::process::exit(1); }
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
    binary.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(cache / "target/release/marja-selfplay.exe", binary)
    shutil.copy2(harness / "Cargo.lock", binary.parent / "Cargo.lock")
    metadata["binary_sha256"] = hashlib.sha256(binary.read_bytes()).hexdigest()
    metadata["lock_sha256"] = hashlib.sha256((harness / "Cargo.lock").read_bytes()).hexdigest()
    (binary.parent / "build.json").write_text(json.dumps(metadata, indent=2), encoding="utf-8")
    return binary, metadata
