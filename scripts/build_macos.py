"""Build a local, ad-hoc signed native YM_Link.app (not a distribution release)."""

from pathlib import Path
import argparse
import os
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def run(args, *, capture=False):
    return subprocess.run(
        args, cwd=ROOT, check=True, timeout=900,
        env={**os.environ, "MACOSX_DEPLOYMENT_TARGET": "14.0", "CARGO_HTTP_MULTIPLEXING": "false"},
        capture_output=capture, text=True,
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sign-identity", default="-", help="local Apple Development identity; default is ad-hoc")
    args = parser.parse_args()
    if sys.platform != "darwin":
        raise SystemExit("Native macOS builds require macOS and Xcode.")
    run(["cargo", "build", "--release", "--locked", "-p", "ymlink-desktop", "--lib"])
    run(["swift", "build", "--package-path", "macos", "-c", "release"])
    binaries = Path(run(["swift", "build", "--package-path", "macos", "-c", "release", "--show-bin-path"], capture=True).stdout.strip())
    app = ROOT / "dist/YM_Link.app"
    if app.exists():
        shutil.rmtree(app)
    executable = app / "Contents/MacOS"
    resources = app / "Contents/Resources"
    executable.mkdir(parents=True)
    resources.mkdir(parents=True)
    shutil.copy2(binaries / "YM_Link", executable / "YM_Link")
    shutil.copy2(ROOT / "macos/Info.plist", app / "Contents/Info.plist")
    for bundle in binaries.glob("*.bundle"):
        shutil.copytree(bundle, resources / bundle.name)
    for locale in (ROOT / "macos/Resources").glob("*.lproj"):
        shutil.copytree(locale, resources / locale.name)
    run(["codesign", "--force", "--sign", args.sign_identity, str(app)])
    run(["codesign", "--verify", "--strict", str(app)])
    print(f"Built {app.relative_to(ROOT)} (local development signing; not notarized).")


if __name__ == "__main__":
    main()
