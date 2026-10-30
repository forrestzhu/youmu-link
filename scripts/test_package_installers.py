"""Installer payload, portability and failure regression tests (no compiler required)."""

import hashlib
import json
from pathlib import Path
import stat
import sys
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import package_installers as installers


class InstallerTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.payload = self.root / "payload"
        self.payload.mkdir()
        self.archive = self.root / "YM_Link-0.1.0-macos-arm64-abcdefabcdef.zip"
        self.revision = "abcdefabcdef0123456789012345678901234567"
        self.write("BUILD-INFO.json", json.dumps({"version": "0.1.0", "revision": self.revision}))
        for name in ("LICENSE", "NOTICE", "THIRD_PARTY_NOTICES.md", "licenses/example/LICENSE"):
            self.write(name, "license text\n")
        self.write("YM_Link.app/Contents/MacOS/YM_Link", "executable")
        self.write("YM_Link.app/Contents/Resources/en.lproj/Localizable.strings", "localized")

    def write(self, name, data):
        path = self.payload / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(data, encoding="utf-8")
        return path

    def test_installed_payload_corruption_is_detected(self):
        import shutil
        installed = self.root / "installed"
        shutil.copytree(self.payload, installed)
        installers.verify_payload(self.payload, installed)
        (installed / "licenses/example/LICENSE").write_text("changed", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "licenses/example/LICENSE"):
            installers.verify_payload(self.payload, installed)
        (installed / "licenses/example/LICENSE").unlink()
        with self.assertRaisesRegex(ValueError, "licenses/example/LICENSE"):
            installers.verify_payload(self.payload, installed)

    def test_zip_executable_mode_survives_installer_staging(self):
        with zipfile.ZipFile(self.archive, "w") as zipped:
            entry = zipfile.ZipInfo(self.archive.stem + "/YM_Link.app/Contents/MacOS/YM_Link")
            entry.external_attr = (stat.S_IFREG | 0o755) << 16
            zipped.writestr(entry, b"executable")
        with patch.object(installers.sys, "platform", "darwin"):
            payload = installers.extract_payload(self.archive, self.root / "stage")
        if sys.platform != "win32":
            self.assertEqual(stat.S_IMODE((payload / "YM_Link.app/Contents/MacOS/YM_Link").stat().st_mode), 0o755)

    def test_zip_cannot_escape_installer_staging(self):
        with zipfile.ZipFile(self.archive, "w") as zipped:
            zipped.writestr("../../outside.txt", "private")
        with self.assertRaisesRegex(ValueError, "staging"):
            installers.extract_payload(self.archive, self.root / "stage")
        self.assertFalse((self.root / "outside.txt").exists())

    def test_wrong_host_never_creates_a_package(self):
        with patch.object(installers.sys, "platform", "linux"), patch.object(installers, "package_build") as archive:
            with self.assertRaisesRegex(ValueError, "native operating system"):
                installers.package_installers("windows-x64", self.revision)
            archive.assert_not_called()

    def test_checksum_names_exact_download_asset(self):
        artifact = self.root / "preview-setup.exe"
        artifact.write_bytes(b"installer")
        installers.write_checksum(artifact)
        self.assertEqual(artifact.with_suffix(".exe.sha256").read_text(encoding="utf-8"),
                         hashlib.sha256(b"installer").hexdigest() + "  preview-setup.exe\n")

    def test_missing_compiler_output_is_not_published(self):
        with patch.object(installers, "find_iscc", return_value="ISCC.exe"), patch.object(installers, "run"):
            with self.assertRaisesRegex(ValueError, "expected installer"):
                installers.build_windows(self.archive, self.payload, self.root)
        self.assertFalse(list(self.root.glob("*-setup.exe")))

    @unittest.skipIf(sys.platform == "win32", "macOS symlink behavior is tested on Unix hosts")
    def test_dmg_app_keeps_licenses_after_dragging_and_failed_verification_detaches(self):
        calls = []
        mounted = self.root / "Mounted Image"

        def native(arguments, timeout=300):
            import shutil
            calls.append(arguments)
            if arguments[:2] == ["hdiutil", "create"]:
                Path(arguments[-1]).write_bytes(b"disk image")
            if arguments[:2] == ["hdiutil", "attach"]:
                shutil.copytree(self.payload, mounted, dirs_exist_ok=True, symlinks=True)

        with patch.object(installers, "run", side_effect=native), \
             patch.object(installers.os.path, "ismount", return_value=True), \
             patch.object(installers, "verify_payload", side_effect=ValueError("corrupt mounted payload")):
            with self.assertRaisesRegex(ValueError, "corrupt mounted payload"):
                installers.build_macos(self.archive, self.payload, self.root, smoke_test=True)
        self.assertIn(["hdiutil", "detach", str(mounted)], calls)
        legal = self.payload / "YM_Link.app/Contents/Resources/legal"
        self.assertEqual((legal / "licenses/example/LICENSE").read_bytes(), (self.payload / "licenses/example/LICENSE").read_bytes())
        self.assertEqual((legal / "BUILD-INFO.json").read_bytes(), (self.payload / "BUILD-INFO.json").read_bytes())
        self.assertIn(["codesign", "--force", "--sign", "-", str(self.payload / "YM_Link.app")], calls)
        self.assertFalse(self.archive.with_suffix(".dmg.sha256").exists())


if __name__ == "__main__":
    unittest.main()
