"""Developer archive regression tests; run with `python scripts/test_package_build.py`."""

import hashlib
import json
from pathlib import Path
import plistlib
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest
import zipfile


class PackageBuildTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        (self.root / "scripts").mkdir()
        self.script = self.root / "scripts/package_build.py"
        for filename in ("package_build.py", "build_privacy.py", "app_icons.py"):
            source = Path(__file__).with_name(filename)
            if source.exists():
                shutil.copy2(source, self.script.with_name(filename))
        (self.root / "Cargo.toml").write_text('[workspace.package]\nversion = "0.1.0"\n', encoding="utf-8")
        for name in ("LICENSE", "NOTICE", "THIRD_PARTY_NOTICES.md", "licenses/example/LICENSE"):
            self.write("dist/legal/" + name, "legal text\n")
        for name in ("YM_Link.exe", "ymlink_desktop.dll", "coreclr.dll", "hostfxr.dll", "PresentationFramework.dll"):
            self.write("dist/YM_Link/" + name, "binary")
        self.write("assets/app-icon/YM_Link.icns", "icon fixture")

    def write(self, name, content):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
        return path

    def package(self, platform="windows-x64", revision="abcdefabcdef0123456789012345678901234567"):
        return subprocess.run(
            [sys.executable, str(self.script), "--platform", platform, "--revision", revision],
            cwd=self.root, capture_output=True, text=True, timeout=30,
        )

    def mac_app(self):
        binary = self.write("dist/YM_Link.app/Contents/MacOS/YM_Link", "executable")
        metadata = self.root / "dist/YM_Link.app/Contents/Info.plist"
        metadata.write_bytes(plistlib.dumps({"CFBundleIconFile": "YM_Link.icns"}))
        self.write("dist/YM_Link.app/Contents/Resources/YM_Link.icns", "icon fixture")
        self.write("dist/YM_Link.app/Contents/_CodeSignature/CodeResources", "signature")
        return binary

    def test_windows_archive_contains_only_build_and_legal_files_with_verified_checksum(self):
        self.write(".identity.json", "private identity")
        self.write("dist/private-state.json", "private state")
        result = self.package()
        self.assertEqual(result.returncode, 0, result.stderr)
        name = "YM_Link-0.1.0-windows-x64-abcdefabcdef"
        archive = self.root / f"dist/packages/{name}.zip"
        with zipfile.ZipFile(archive) as packaged:
            self.assertEqual(set(packaged.namelist()), {
                f"{name}/{filename}" for filename in (
                    "YM_Link.exe", "ymlink_desktop.dll", "coreclr.dll", "hostfxr.dll", "PresentationFramework.dll", "LICENSE", "NOTICE",
                    "THIRD_PARTY_NOTICES.md", "licenses/example/LICENSE", "BUILD-INFO.json",
                )
            })
            metadata = json.loads(packaged.read(f"{name}/BUILD-INFO.json"))
            self.assertEqual(metadata["revision"], "abcdefabcdef0123456789012345678901234567")
            self.assertEqual(metadata["platform"], "windows-x64")
            self.assertNotIn(str(self.root), json.dumps(metadata))
        expected = hashlib.sha256(archive.read_bytes()).hexdigest()
        self.assertEqual(archive.with_suffix(".zip.sha256").read_text(encoding="utf-8"),
                         f"{expected}  {name}.zip\n")

    def test_missing_native_backend_prevents_archive(self):
        (self.root / "dist/YM_Link/ymlink_desktop.dll").unlink()
        result = self.package()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("ymlink_desktop.dll", result.stderr)
        self.assertFalse(list((self.root / "dist").glob("packages/*.zip")))

    def test_own_binaries_with_local_build_paths_cannot_be_packaged_or_log_the_path(self):
        for filename, encoding in (("YM_Link.exe", "utf-8"), ("YM_Link.dll", "utf-16le"), ("ymlink_desktop.dll", "utf-8")):
            with self.subTest(filename=filename, encoding=encoding):
                binary = self.root / "dist/YM_Link" / filename
                private = str(self.root).replace("\\", "/").upper() + "/src/private.rs"
                binary.write_bytes(b"binary\x00" + private.encode(encoding))
                result = self.package()
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("build-machine path", result.stderr)
                self.assertIn(filename, result.stderr)
                self.assertNotIn(str(self.root), result.stderr)
                self.assertNotIn(private, result.stderr)
                self.assertFalse(list((self.root / "dist").glob("packages/*.zip")))
                binary.write_bytes(b"binary")

    def test_vendor_public_build_paths_are_not_reported_as_local_secrets(self):
        self.write("dist/YM_Link/coreclr.dll", "D:/public-ci/runtime/src/example.cpp")
        self.write("dist/legal/licenses/example/LICENSE", "C:/example/public/license/path")
        result = self.package()
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_mac_main_binary_with_local_build_path_cannot_be_packaged(self):
        self.mac_app().write_text(str(self.root) + "/macos/main.swift", encoding="utf-8")
        result = self.package("macos-arm64")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("build-machine path", result.stderr)
        self.assertNotIn(str(self.root), result.stderr)

    def test_missing_macos_icon_prevents_archive(self):
        self.mac_app()
        (self.root / "dist/YM_Link.app/Contents/Resources/YM_Link.icns").unlink()
        result = self.package("macos-arm64")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("YM_Link.icns", result.stderr)
        self.assertFalse(list((self.root / "dist").glob("packages/*.zip")))

    def test_stale_macos_icon_and_wrong_plist_reference_prevent_archive(self):
        self.mac_app()
        self.write("dist/YM_Link.app/Contents/Resources/YM_Link.icns", "stale icon")
        result = self.package("macos-arm64")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("differs from the committed application icon", result.stderr)
        self.write("dist/YM_Link.app/Contents/Resources/YM_Link.icns", "icon fixture")
        (self.root / "dist/YM_Link.app/Contents/Info.plist").write_bytes(plistlib.dumps({}))
        result = self.package("macos-arm64")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("CFBundleIconFile", result.stderr)
        self.assertFalse(list((self.root / "dist").glob("packages/*.zip")))

    def test_directory_links_cannot_include_files_outside_either_archive_input(self):
        private = self.write("private/identity.json", "private fixture").parent
        for directory in ("dist/YM_Link", "dist/legal/licenses"):
            with self.subTest(directory=directory):
                link = self.root / directory / "unexpected-link"
                if sys.platform == "win32":
                    quoted = lambda path: "'" + str(path).replace("'", "''") + "'"
                    subprocess.run(["powershell.exe", "-NoProfile", "-NonInteractive", "-Command", f"New-Item -ItemType Junction -Path {quoted(link)} -Target {quoted(private)} | Out-Null"], check=True, capture_output=True)
                else:
                    link.symlink_to(private, target_is_directory=True)
                result = self.package()
                self.assertNotEqual(result.returncode, 0, "a directory link must not disclose external files")
                self.assertFalse(list((self.root / "dist").glob("packages/*.zip")))
                if link.is_symlink():
                    link.unlink()
                else:
                    link.rmdir()

    def test_missing_legal_text_prevents_archive(self):
        (self.root / "dist/legal/NOTICE").unlink()
        result = self.package()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("NOTICE", result.stderr)

    def test_revision_cannot_escape_output_directory(self):
        result = self.package(revision="../../outside")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("revision", result.stderr)
        self.assertFalse((self.root / "outside.zip").exists())

    @unittest.skipIf(sys.platform == "win32", "Windows does not expose Unix executable modes")
    def test_mac_archive_preserves_executable_mode_and_rejects_symlinks(self):
        binary = self.mac_app()
        binary.chmod(0o755)
        result = self.package("macos-arm64")
        self.assertEqual(result.returncode, 0, result.stderr)
        archive = next((self.root / "dist/packages").glob("*.zip"))
        with zipfile.ZipFile(archive) as packaged:
            info = packaged.getinfo("YM_Link-0.1.0-macos-arm64-abcdefabcdef/YM_Link.app/Contents/MacOS/YM_Link")
            self.assertEqual(stat.S_IMODE(info.external_attr >> 16), 0o755)
            self.assertEqual(packaged.read("YM_Link-0.1.0-macos-arm64-abcdefabcdef/YM_Link.app/Contents/Resources/YM_Link.icns"),
                             (self.root / "assets/app-icon/YM_Link.icns").read_bytes())
        (binary.parent / "unexpected-link").symlink_to(self.root / ".identity.json")
        result = self.package("macos-arm64")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("symlink", result.stderr)


if __name__ == "__main__":
    unittest.main()
