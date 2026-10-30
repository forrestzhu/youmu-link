"""Build a local, ad-hoc signed native YM_Link.app (not a distribution release)."""

from pathlib import Path
import argparse
import re
import shutil
import subprocess
import sys
import tempfile

from build_privacy import rust_build_environment
from app_icons import verify_macos_icon

ROOT = Path(__file__).resolve().parents[1]


def run(args, *, capture=False, timeout=900):
    return subprocess.run(
        args, cwd=ROOT, check=True, timeout=timeout,
        env={**rust_build_environment(ROOT), "MACOSX_DEPLOYMENT_TARGET": "14.0", "CARGO_HTTP_MULTIPLEXING": "false"},
        capture_output=capture, text=True,
    )


def verify_runtime_dependencies(libraries):
    if any(not library.startswith(("/System/Library/", "/usr/lib/"))
           and not re.fullmatch(r"@rpath/libswift[\w]+\.dylib", library) for library in libraries):
        raise ValueError("The app links a non-system library; Rust must be linked statically.")


def build_swift_package(package):
    # SwiftPM does not track archives passed through -Xlinker as build inputs.
    # Clear cached products so a Rust-only edit cannot ship an old backend.
    run(["swift", "package", "--package-path", str(package), "clean"])
    run(["swift", "build", "--package-path", str(package), "-c", "release"])
    return Path(run(["swift", "build", "--package-path", str(package), "-c", "release", "--show-bin-path"], capture=True).stdout.strip())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sign-identity", default="-", help="local Apple Development identity; default is ad-hoc")
    args = parser.parse_args()
    if sys.platform != "darwin":
        raise SystemExit("Native macOS builds require macOS and Xcode.")
    run(["cargo", "build", "--release", "--locked", "-p", "ymlink-desktop", "--lib"])
    binaries = build_swift_package(ROOT / "macos")
    app = ROOT / "dist/YM_Link.app"
    if app.resolve().parent != ROOT / "dist":
        raise SystemExit("The app output must stay inside the workspace dist directory.")
    if app.exists():
        shutil.rmtree(app)
    executable = app / "Contents/MacOS"
    resources = app / "Contents/Resources"
    executable.mkdir(parents=True)
    resources.mkdir(parents=True)
    shutil.copy2(binaries / "YM_Link", executable / "YM_Link")
    shutil.copy2(ROOT / "macos/Info.plist", app / "Contents/Info.plist")
    icon = ROOT / "assets/app-icon/YM_Link.icns"
    shutil.copy2(icon, resources / icon.name)
    for bundle in binaries.glob("*.bundle"):
        shutil.copytree(bundle, resources / bundle.name)
    for locale in (ROOT / "macos/Resources").glob("*.lproj"):
        shutil.copytree(locale, resources / locale.name)
    verify_macos_icon(app, icon)
    dependencies = run(["otool", "-L", str(executable / "YM_Link")], capture=True).stdout.splitlines()[1:]
    verify_runtime_dependencies([line.strip().split(" (", 1)[0] for line in dependencies])
    # Remove debug/source records before signing; runtime resource paths are
    # resolved relative to the app rather than rewritten in the binary.
    run(["xcrun", "strip", "-S", str(executable / "YM_Link")])
    run(["codesign", "--force", "--sign", args.sign_identity, str(app)])
    run(["codesign", "--verify", "--strict", str(app)])
    with tempfile.TemporaryDirectory(prefix="ym-link-resources-") as temporary:
        relocated = Path(temporary) / app.name
        shutil.copytree(app, relocated)
        verify_macos_icon(relocated, icon)
        run([str(relocated / "Contents/MacOS/YM_Link"), "--verify-localizations"], timeout=30)
    print(f"Built {app.relative_to(ROOT)} (local development signing; not notarized).")


if __name__ == "__main__":
    main()
