"""Build unsigned Windows x64 EXE or ad-hoc signed macOS arm64 DMG previews.

Uses exactly the payload validated by package_build.py, and also retains its ZIP.
Windows --smoke-test performs a per-user install/uninstall; use a disposable CI host.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import zipfile

from package_build import ROOT, package_build
from app_icons import verify_windows_icon


def run(arguments, timeout=300):
    return subprocess.run(arguments, cwd=ROOT, check=True, timeout=timeout)


def checksum(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_checksum(path):
    path.with_suffix(path.suffix + ".sha256").write_text(
        f"{checksum(path)}  {path.name}\n", encoding="utf-8", newline="\n",
    )


def extract_payload(archive, target):
    """Extract our freshly validated archive while retaining executable permissions."""
    with zipfile.ZipFile(archive) as zipped:
        for entry in zipped.infolist():
            destination = target / entry.filename
            if not destination.resolve().is_relative_to(target.resolve()):
                raise ValueError("archive member must stay inside the installer staging directory")
            zipped.extract(entry, target)
            if sys.platform != "win32":
                destination.chmod((entry.external_attr >> 16) & 0o777)
    return target / archive.stem


def verify_payload(expected, installed):
    """An installer may add its own files, but must preserve every payload byte."""
    for source in expected.rglob("*"):
        if source.is_file():
            target = installed / source.relative_to(expected)
            if not target.is_file() or checksum(source) != checksum(target):
                raise ValueError("installed payload differs: " + source.relative_to(expected).as_posix())


def find_iscc(explicit=None):
    candidates = [explicit] if explicit else [
        shutil.which("ISCC.exe"),
        str(Path(os.environ.get("ProgramFiles(x86)", "C:/Program Files (x86)")) / "Inno Setup 6/ISCC.exe"),
    ]
    for candidate in candidates:
        if candidate and Path(candidate).is_file():
            return str(candidate)
    raise ValueError("Inno Setup 6 ISCC.exe is required; use --iscc to select the compiler")


def verify_windows_install_state(installed, *, present, desktop_icon):
    """Inspect real Shell links and process state on the disposable Windows host."""
    target = str(installed / "YM_Link.exe").replace("'", "''")
    boolean = lambda value: "$true" if value else "$false"
    script = f"""
$ErrorActionPreference = 'Stop'
$target = '{target}'
$desktop = Join-Path ([Environment]::GetFolderPath('DesktopDirectory')) 'YM_Link.lnk'
$menu = Join-Path ([Environment]::GetFolderPath('Programs')) 'Youmu\\YM_Link.lnk'
$shell = New-Object -ComObject WScript.Shell
foreach ($entry in @(@($desktop, {boolean(desktop_icon)}), @($menu, {boolean(present)}))) {{
    if ((Test-Path -LiteralPath $entry[0]) -ne $entry[1]) {{ throw 'Installer shortcut presence mismatch' }}
    if ($entry[1] -and $shell.CreateShortcut($entry[0]).TargetPath -ne $target) {{ throw 'Installer shortcut target mismatch' }}
}}
$key = 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Youmu.YM_Link.Preview.x64_is1'
if ((Test-Path -LiteralPath $key) -ne {boolean(present)}) {{ throw 'Installer registration presence mismatch' }}
$running = @(Get-CimInstance Win32_Process -Filter "Name='YM_Link.exe'" | Where-Object {{ $_.ExecutablePath -eq $target }})
if ($running.Count) {{ throw 'Silent installation unexpectedly launched the application' }}
"""
    run(["powershell.exe", "-NoProfile", "-NonInteractive", "-Command", script], timeout=30)


def smoke_windows(installer, payload, temporary):
    # Protect a developer's existing install from a test that uses the same AppId.
    import winreg

    key = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Youmu.YM_Link.Preview.x64_is1"
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, key, 0, winreg.KEY_READ | winreg.KEY_WOW64_64KEY):
            raise ValueError("installer smoke test requires a user without an existing YM_Link preview installation")
    except FileNotFoundError:
        pass
    installed = temporary / "Install Root"
    options = ["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/RESTARTEXITCODE=3010"]
    # Refuse unrelated pre-existing shortcuts as well as a registered install.
    verify_windows_install_state(installed, present=False, desktop_icon=False)
    for tasks, desktop_icon in (([], True), (["/TASKS="], False)):
        try:
            run([str(installer), "/SP-", *options, "/NOCLOSEAPPLICATIONS", "/LANG=english", f"/DIR={installed}", *tasks])
            verify_payload(payload, installed)
            verify_windows_install_state(installed, present=True, desktop_icon=desktop_icon)
            run([str(installed / "YM_Link.exe"), "--smoke-test"], timeout=30)
        finally:
            uninstaller = installed / "unins000.exe"
            if uninstaller.is_file():
                run([str(uninstaller), "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"], timeout=120)
        for _ in range(100):
            if not (installed / "YM_Link.exe").exists():
                break
            time.sleep(0.1)
        else:
            raise ValueError("installer smoke test did not remove the installed executable")
        verify_windows_install_state(installed, present=False, desktop_icon=False)
    print("Verified Windows install, complete payload, optional desktop and Start menu shortcuts, silent no-launch, native startup/shutdown and uninstall cleanup.")


def build_windows(archive, payload, temporary, *, iscc=None, smoke_test=False):
    metadata = json.loads((payload / "BUILD-INFO.json").read_text(encoding="utf-8"))
    name = archive.stem + "-setup"
    # The compiler's absolute input paths stay in this ignored, temporary script invocation.
    run([
        find_iscc(iscc), "/Qp", f"/DPayloadDir={payload}", f"/DOutputDir={temporary}",
        f"/DOutputName={name}", f"/DAppVersion={metadata['version']}", f"/DRevision={metadata['revision'][:12]}",
        str(ROOT / "scripts/installer_windows.iss"),
    ])
    installer = temporary / (name + ".exe")
    if not installer.is_file():
        raise ValueError("Inno Setup did not produce the expected installer")
    verify_windows_icon(installer, ROOT / "assets/app-icon/YM_Link.ico")
    if smoke_test:
        smoke_windows(installer, payload, temporary)
    result = archive.with_name(installer.name)
    installer.replace(result)
    return result


def build_macos(archive, payload, temporary, *, smoke_test=False):
    app = payload / "YM_Link.app"
    # Licenses travel with the app when users drag it into Applications.
    legal = app / "Contents/Resources/legal"
    legal.mkdir(parents=True)
    for name in ("LICENSE", "NOTICE", "THIRD_PARTY_NOTICES.md", "BUILD-INFO.json"):
        shutil.copy2(payload / name, legal / name)
    shutil.copytree(payload / "licenses", legal / "licenses")
    # Adding resources changes the code seal; re-sign this preview with an ad-hoc identity.
    run(["codesign", "--force", "--sign", "-", str(app)])
    run(["codesign", "--verify", "--strict", str(app)])
    (payload / "Applications").symlink_to("/Applications", target_is_directory=True)
    (payload / "INSTALL.txt").write_text(
        "YM_Link Development Preview\n\n"
        "Drag YM_Link.app to Applications, then eject this disk image.\n"
        "Requires Apple Silicon; build target macOS 14 or later.\n"
        "This development preview is ad-hoc signed and is not notarized.\n"
        "macOS may block it. Only open it if you trust its source.\n"
        "Third-party licenses are included inside the app in Contents/Resources/legal.\n",
        encoding="utf-8", newline="\n",
    )
    image = temporary / (archive.stem + ".dmg")
    run(["hdiutil", "create", "-volname", "YM_Link Preview", "-srcfolder", str(payload),
         "-fs", "HFS+", "-format", "UDZO", str(image)])
    run(["hdiutil", "verify", str(image)])
    mount = temporary / "Mounted Image"
    mount.mkdir()
    try:
        run(["hdiutil", "attach", "-readonly", "-nobrowse", "-mountpoint", str(mount), str(image)])
        verify_payload(app, mount / "YM_Link.app")
        for name in ("LICENSE", "NOTICE", "THIRD_PARTY_NOTICES.md", "BUILD-INFO.json"):
            if checksum(payload / name) != checksum(mount / name):
                raise ValueError("disk image lost a required legal or build metadata file")
        if not (mount / "Applications").is_symlink() or os.readlink(mount / "Applications") != "/Applications":
            raise ValueError("disk image must provide the Applications shortcut")
        run(["codesign", "--verify", "--strict", str(mount / "YM_Link.app")])
        if smoke_test:
            # Mimic drag-to-install without touching the user's Applications folder.
            installed = temporary / "Installed App/YM_Link.app"
            shutil.copytree(mount / "YM_Link.app", installed)
            run(["codesign", "--verify", "--strict", str(installed)])
            run([str(installed / "Contents/MacOS/YM_Link"), "--verify-localizations"], timeout=30)
            print("Verified mounted DMG payload and relocated app resources/signature.")
    finally:
        if os.path.ismount(mount):
            run(["hdiutil", "detach", str(mount)], timeout=60)
    result = archive.with_suffix(".dmg")
    image.replace(result)
    return result


def package_installers(platform, revision, *, iscc=None, smoke_test=False):
    expected_host = "win32" if platform == "windows-x64" else "darwin"
    if sys.platform != expected_host:
        raise ValueError(f"{platform} installers must be built on their native operating system")
    archive = package_build(platform, revision)
    with tempfile.TemporaryDirectory(prefix="installer-", dir=archive.parent) as directory:
        temporary = Path(directory)
        payload = extract_payload(archive, temporary / "payload")
        if platform == "windows-x64":
            result = build_windows(archive, payload, temporary, iscc=iscc, smoke_test=smoke_test)
        else:
            result = build_macos(archive, payload, temporary, smoke_test=smoke_test)
    write_checksum(result)
    print(f"Packaged {result.relative_to(ROOT).as_posix()} and SHA-256 checksum.")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--platform", choices=("windows-x64", "macos-arm64"), required=True)
    parser.add_argument("--revision", help="full Git commit SHA; defaults to checked-out HEAD")
    parser.add_argument("--iscc", help="path to the Windows Inno Setup compiler")
    parser.add_argument("--smoke-test", action="store_true", help="test installed app on a disposable CI host")
    args = parser.parse_args()
    revision = args.revision or subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, check=True, capture_output=True, text=True,
    ).stdout.strip()
    try:
        package_installers(args.platform, revision, iscc=args.iscc, smoke_test=args.smoke_test)
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        # Avoid dumping native tool invocations containing absolute user/build paths.
        if isinstance(error, ValueError):
            parser.exit(1, f"Packaging failed: {error}\n")
        parser.exit(1, "Packaging failed: native packaging or verification command failed.\n")


if __name__ == "__main__":
    main()
