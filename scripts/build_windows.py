"""Build the native Windows x64 YM_Link app (local development, unsigned)."""

import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def run(arguments, timeout=900):
    subprocess.run(
        arguments, cwd=ROOT, check=True, timeout=timeout,
        env={**os.environ, "CARGO_HTTP_MULTIPLEXING": "false"},
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--debug", action="store_true", help="use unoptimized Rust and .NET builds")
    parser.add_argument("--framework-dependent", action="store_true", help="require the installed .NET 8 Windows Desktop Runtime")
    parser.add_argument("--smoke-test", action="store_true", help="verify bounded real backend startup, snapshot and shutdown")
    args = parser.parse_args()
    if sys.platform != "win32":
        raise SystemExit("Native Windows builds require Windows, Rust MSVC and the .NET 8 SDK.")
    configuration = "Debug" if args.debug else "Release"
    cargo = ["cargo", "build", "--locked", "-p", "ymlink-desktop", "--lib"]
    if not args.debug:
        cargo.append("--release")
    run(cargo)
    native = ROOT / "target" / configuration.lower() / "ymlink_desktop.dll"
    run(["dotnet", "run", "--project", "windows/Tests/YM_Link.Tests.csproj", "-c", configuration, "--", "--native", str(native)])
    output = ROOT / "dist/YM_Link"
    if output.resolve().parent != ROOT / "dist":
        raise SystemExit("The app output must stay inside the workspace dist directory.")
    if output.exists():
        shutil.rmtree(output)
    output.mkdir(parents=True)
    run([
        "dotnet", "publish", "windows/YM_Link/YM_Link.csproj", "-c", configuration,
        "-r", "win-x64", "--self-contained", "false" if args.framework_dependent else "true",
        "-p:DebugType=None", "-p:DebugSymbols=false", "-o", str(output),
    ])
    # Native ABI is loaded beside the EXE, never from a search path or a user folder.
    shutil.copy2(native, output / "ymlink_desktop.dll")
    if args.smoke_test:
        run([str(output / "YM_Link.exe"), "--smoke-test"], timeout=30)
        print("Smoke test passed: real Rust startup, snapshot and clean shutdown.")
    print(f"Built {output.relative_to(ROOT)} (Windows x64; local unsigned build).")


if __name__ == "__main__":
    main()
