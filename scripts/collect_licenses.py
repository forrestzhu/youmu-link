"""Collect upstream license texts for the locked dependency graph and restored runtime."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
LEGAL_FILES = ("LICENSE", "NOTICE", "THIRD_PARTY_NOTICES.md")


def token(value: str) -> str:
    if not isinstance(value, str) or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._+\-]*", value):
        raise ValueError("invalid dependency name or version")
    return value


def license_files(directory: Path, explicit: str | None = None) -> list[Path]:
    files = set()
    for item in directory.iterdir():
        if item.name.lower().startswith(("license", "licence", "copying", "copyright", "notice", "unlicense")):
            files.update([item] if item.is_file() else (p for p in item.rglob("*") if p.is_file()))
    if explicit:
        files.add(directory / explicit)
    base = directory.resolve()
    if any(not item.is_file() or not item.resolve().is_relative_to(base) for item in files):
        raise ValueError("dependency license file is missing or outside its package")
    if not any(item.name.lower().startswith(("license", "licence", "copying", "unlicense")) or explicit and item == directory / explicit for item in files):
        raise ValueError("dependency has no license text")
    return sorted(files)


def copy_texts(source: Path, target: Path, files: list[Path]) -> list[str]:
    result = []
    for item in files:
        relative = item.relative_to(source)
        output = target / relative
        output.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(item, output)
        result.append(relative.as_posix())
    return result


def collect_rust(packages: list[dict], destination: Path) -> list[dict]:
    inventory = []
    for package in sorted(packages, key=lambda item: (item["name"], item["version"])):
        if package.get("source") is None:
            continue
        name, version = token(package["name"]), token(package["version"])
        if not package.get("license") and not package.get("license_file"):
            raise ValueError(f"{name} {version}: missing license metadata")
        source = Path(package["manifest_path"]).parent
        files = license_files(source, package.get("license_file"))
        copied = copy_texts(source, destination / "rust" / f"{name}-{version}", files)
        inventory.append({"name": name, "version": version, "license": package.get("license"), "files": copied})
    if not inventory:
        raise ValueError("no third-party Rust dependencies collected")
    return inventory


def collect_windows(assets: dict, destination: Path) -> list[dict]:
    restored = {}
    prefixes = ("microsoft.netcore.app.runtime.", "microsoft.windowsdesktop.app.runtime.")
    for framework in assets["project"]["frameworks"].values():
        for dependency in framework.get("downloadDependencies", []):
            name = dependency["name"].lower()
            if not name.startswith(prefixes):
                continue
            parts = dependency["version"].strip("[]").split(",")
            if len(parts) != 2 or parts[0].strip() != parts[1].strip():
                raise ValueError("runtime pack must resolve to an exact version")
            version = token(parts[0].strip())
            name = token(name)
            if name in restored and restored[name] != version:
                raise ValueError("conflicting runtime pack versions")
            restored[name] = version
    if not all(any(name.startswith(prefix) for name in restored) for prefix in prefixes):
        raise ValueError("missing restored Core or Windows Desktop runtime pack")
    inventory = []
    for name, version in sorted(restored.items()):
        candidates = [Path(folder) / name / version for folder in assets["packageFolders"]]
        source = next((folder for folder in candidates if folder.is_dir()), None)
        if source is None:
            raise ValueError(f"{name} {version}: runtime pack not restored")
        files = license_files(source)
        if name.startswith(prefixes[0]) and not any("third-party-notices" in item.name.lower() for item in files):
            # The runtime pack names this file THIRD-PARTY-NOTICES, not NOTICE.
            notices = source / "THIRD-PARTY-NOTICES.TXT"
            if not notices.is_file():
                raise ValueError("Core runtime third-party notices are missing")
            files.append(notices)
        copied = copy_texts(source, destination / "dotnet" / name / version, files)
        inventory.append({"name": name, "version": version, "license": "MIT and bundled third-party terms", "files": copied})
    return inventory


def collect_toolchain(sysroot: Path, version: str, destination: Path) -> dict:
    version = token(version)
    source = sysroot / "share/doc/rust"
    required = ("LICENSE-APACHE", "LICENSE-MIT", "COPYRIGHT")
    if not all((source / name).is_file() for name in required):
        raise ValueError(f"Rust {version}: standard library/runtime license texts are missing")
    files = license_files(source)
    copied = copy_texts(source, destination / "rust-toolchain" / version, files)
    return {"name": "Rust standard library and runtime", "version": version,
            "license": "MIT OR Apache-2.0 and bundled third-party terms", "files": copied}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "dist/legal")
    parser.add_argument("--windows-assets", type=Path)
    args = parser.parse_args()
    owned_dist = ROOT / "dist"
    dist = owned_dist.resolve()
    output = args.output.resolve()
    if dist != owned_dist or output == dist or not output.is_relative_to(dist):
        raise ValueError("legal bundle output must be a subdirectory of this repository's dist")
    # Gather first; never replace a prior successful bundle with incomplete data.
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--format-version", "1"], cwd=ROOT,
        text=True, encoding="utf-8",
    ))
    assets = json.loads(args.windows_assets.read_text(encoding="utf-8")) if args.windows_assets else None
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = Path(tempfile.mkdtemp(prefix=".legal-", dir=output.parent))
    try:
        for name in LEGAL_FILES:
            shutil.copyfile(ROOT / name, temporary / name)
        rust = collect_rust(metadata["packages"], temporary / "licenses")
        version_output = subprocess.check_output(["rustc", "--version"], cwd=ROOT, text=True, encoding="utf-8")
        version = re.match(r"rustc ([0-9A-Za-z.+-]+)(?:\s|$)", version_output)
        if version is None:
            raise ValueError("cannot determine the Rust runtime version")
        sysroot = Path(subprocess.check_output(["rustc", "--print", "sysroot"], cwd=ROOT, text=True, encoding="utf-8").strip())
        toolchain = collect_toolchain(sysroot, version[1], temporary / "licenses")
        windows = collect_windows(assets, temporary / "licenses") if assets else []
        (temporary / "licenses/index.json").write_text(json.dumps({"rust": rust, "rust_toolchain": toolchain, "dotnet": windows}, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
        # Both resolved paths are checked above and the staging name is generated
        # inside that same dist subtree; never recursively remove ambient paths.
        if output.exists():
            shutil.rmtree(output)
        temporary.rename(output)
        print(f"Collected license texts: {len(rust)} Rust packages, Rust runtime {version[1]}, {len(windows)} .NET runtime packs.")
    finally:
        if temporary.exists():
            shutil.rmtree(temporary)


if __name__ == "__main__":
    main()
