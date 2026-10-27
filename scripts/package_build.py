"""Archive an unsigned developer build plus its collected legal notices (Python 3.10+)."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def package_build(platform, revision):
    if not re.fullmatch(r"[0-9a-fA-F]{40}", revision):
        raise ValueError("revision must be a full 40-character Git commit SHA")
    # This workspace declares its single version in the root manifest.
    version_line = re.search(r'(?m)^version\s*=\s*"([0-9A-Za-z.+-]+)"\s*$',
                             (ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    if not version_line:
        raise ValueError("Cargo.toml must declare the workspace version")
    version = version_line.group(1)
    source = ROOT / ("dist/YM_Link" if platform == "windows-x64" else "dist/YM_Link.app")
    legal = ROOT / "dist/legal"
    required = ("YM_Link.exe", "ymlink_desktop.dll", "coreclr.dll", "hostfxr.dll", "PresentationFramework.dll") if platform == "windows-x64" else (
        "Contents/MacOS/YM_Link", "Contents/Info.plist", "Contents/_CodeSignature/CodeResources",
    )
    for path in [*(source / name for name in required),
                 *(legal / name for name in ("LICENSE", "NOTICE", "THIRD_PARTY_NOTICES.md"))]:
        if not path.is_file():
            raise ValueError(f"missing required file: {path.relative_to(ROOT).as_posix()}")
    files = []
    for directory in (source, legal):
        base = directory.resolve()
        if not base.is_relative_to(ROOT / "dist"):
            raise ValueError("build and legal inputs must stay inside dist")
        pending = [directory]
        while pending:
            path = pending.pop()
            # Python 3.10/3.11 do not classify Windows junctions as symlinks.
            # Validate before descending, including links back to an ancestor.
            if path.is_symlink() or getattr(path.lstat(), "st_file_attributes", 0) & 0x400:
                raise ValueError("symlinks and Windows reparse points are not supported in developer archives")
            if not path.resolve().is_relative_to(base):
                raise ValueError("archive entries must stay inside their build or legal input")
            relative = path.relative_to(directory)
            if any(part.startswith(".") for part in relative.parts):
                raise ValueError("hidden files are not supported in developer archives")
            if path.is_dir():
                pending.extend(sorted(path.iterdir(), reverse=True))
            elif path.is_file():
                archive_path = relative
                if platform == "macos-arm64" and directory == source:
                    archive_path = Path("YM_Link.app") / relative
                files.append((path, archive_path.as_posix()))

    if not any(path.is_relative_to(legal / "licenses") for path, _ in files):
        raise ValueError("dist/legal/licenses must contain collected dependency license texts")

    name = f"YM_Link-{version}-{platform}-{revision[:12].lower()}"
    output = ROOT / "dist/packages"
    if not output.resolve().is_relative_to(ROOT / "dist"):
        raise ValueError("package output must stay inside dist")
    output.mkdir(parents=True, exist_ok=True)
    archive = output / f"{name}.zip"
    with tempfile.TemporaryDirectory(dir=output) as temporary:
        staged = Path(temporary) / archive.name
        with zipfile.ZipFile(staged, "w", compression=zipfile.ZIP_DEFLATED) as zipped:
            for path, relative in files:
                # ZipFile.write preserves Unix executable modes on the macOS runner.
                zipped.write(path, f"{name}/{relative}")
            zipped.writestr(f"{name}/BUILD-INFO.json", json.dumps({
                "version": version, "revision": revision.lower(), "platform": platform,
                "distribution": "unsigned developer build; macOS ad-hoc signed, not notarized",
            }, indent=2, sort_keys=True) + "\n")
        staged.replace(archive)
    digest = hashlib.sha256()
    with archive.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    archive.with_suffix(".zip.sha256").write_text(f"{digest.hexdigest()}  {archive.name}\n", encoding="utf-8", newline="\n")
    print(f"Packaged {archive.relative_to(ROOT).as_posix()} and SHA-256 checksum.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--platform", choices=("windows-x64", "macos-arm64"), required=True)
    parser.add_argument("--revision", help="full Git commit SHA; defaults to the checked-out HEAD")
    args = parser.parse_args()
    revision = args.revision or subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, check=True, capture_output=True, text=True,
    ).stdout.strip()
    try:
        package_build(args.platform, revision)
    except ValueError as error:
        parser.exit(1, f"Packaging failed: {error}\n")


if __name__ == "__main__":
    main()
