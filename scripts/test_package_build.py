"""Developer archive regression tests; run with `python scripts/test_package_build.py`."""

import hashlib
import json
from pathlib import Path
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
        self.root = Path(temporary.name)
        (self.root / "scripts").mkdir()
        self.script = self.root / "scripts/package_build.py"
        source = Path(__file__).with_name("package_build.py")
        if source.exists():
            shutil.copy2(source, self.script)
        (self.root / "Cargo.toml").write_text('[workspace.package]\nversion = "0.1.0"\n', encoding="utf-8")
        for name in ("LICENSE", "NOTICE", "THIRD_PARTY_NOTICES.md", "licenses/example/LICENSE"):
            self.write("dist/legal/" + name, "legal text\n")
        for name in ("YM_Link.exe", "ymlink_desktop.dll", "coreclr.dll", "hostfxr.dll", "PresentationFramework.dll"):
            self.write("dist/YM_Link/" + name, "binary")

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
        binary = self.write("dist/YM_Link.app/Contents/MacOS/YM_Link", "executable")
        binary.chmod(0o755)
        self.write("dist/YM_Link.app/Contents/Info.plist", "plist")
        self.write("dist/YM_Link.app/Contents/_CodeSignature/CodeResources", "signature")
        result = self.package("macos-arm64")
        self.assertEqual(result.returncode, 0, result.stderr)
        archive = next((self.root / "dist/packages").glob("*.zip"))
        with zipfile.ZipFile(archive) as packaged:
            info = packaged.getinfo("YM_Link-0.1.0-macos-arm64-abcdefabcdef/YM_Link.app/Contents/MacOS/YM_Link")
            self.assertEqual(stat.S_IMODE(info.external_attr >> 16), 0o755)
        (binary.parent / "unexpected-link").symlink_to(self.root / ".identity.json")
        result = self.package("macos-arm64")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("symlink", result.stderr)


if __name__ == "__main__":
    unittest.main()
