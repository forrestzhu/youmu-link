"""Checks legal-bundle completeness and exact-version runtime lookup."""
from pathlib import Path
import json
import os
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from collect_licenses import collect_rust, collect_windows, main


class LicenseTests(unittest.TestCase):
    def test_embedded_rust_runtime_preserves_exact_toolchain_notices(self):
        from collect_licenses import collect_toolchain
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            docs = root / "toolchain/share/doc/rust"
            docs.mkdir(parents=True)
            for name in ("LICENSE-APACHE", "LICENSE-MIT", "COPYRIGHT"):
                (docs / name).write_text("Original Rust runtime " + name, encoding="utf-8")
            result = collect_toolchain(root / "toolchain", "1.85.0", root / "legal")
            self.assertEqual(result["version"], "1.85.0")
            self.assertEqual((root / "legal/rust-toolchain/1.85.0/COPYRIGHT").read_text(), "Original Rust runtime COPYRIGHT")
            (docs / "COPYRIGHT").unlink()
            with self.assertRaises(ValueError):
                collect_toolchain(root / "toolchain", "1.85.0", root / "missing")

    def test_redirected_dist_cannot_replace_existing_repository_directory(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder).resolve()
            legal = root / "legal"
            legal.mkdir()
            marker = legal / "keep.txt"
            marker.write_text("keep", encoding="utf-8")
            for name in ("LICENSE", "NOTICE", "THIRD_PARTY_NOTICES.md"):
                (root / name).write_text("legal text", encoding="utf-8")
            crate = root / "crate"
            crate.mkdir()
            (crate / "LICENSE").write_text("MIT", encoding="utf-8")
            dist = root / "dist"
            if os.name == "nt":
                quoted = lambda path: "'" + str(path).replace("'", "''") + "'"
                subprocess.run(["powershell.exe", "-NoProfile", "-NonInteractive", "-Command", f"New-Item -ItemType Junction -Path {quoted(dist)} -Target {quoted(root)} | Out-Null"], check=True, capture_output=True)
            else:
                dist.symlink_to(root, target_is_directory=True)
            metadata = {"packages": [{"name": "crate", "version": "1.0", "source": "registry+test", "license": "MIT", "manifest_path": str(crate / "Cargo.toml")}]}
            with patch("collect_licenses.ROOT", root), patch("sys.argv", ["collect_licenses.py", "--output", str(dist / "legal")]), patch("collect_licenses.subprocess.check_output", return_value=json.dumps(metadata)) as cargo:
                with self.assertRaises(ValueError):
                    main()
                cargo.assert_not_called()
            self.assertEqual(marker.read_text(), "keep")

    def test_command_rejects_output_outside_owned_dist_before_running_cargo(self):
        with tempfile.TemporaryDirectory() as folder, patch("sys.argv", ["collect_licenses.py", "--output", folder]), patch("collect_licenses.subprocess.check_output") as cargo:
            with self.assertRaises(ValueError):
                main()
            cargo.assert_not_called()

    def test_rust_preserves_text_and_special_license_terms(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            crate = root / "crate"
            crate.mkdir()
            (crate / "Cargo.toml").write_text("", encoding="utf-8")
            (crate / "LICENSE-APACHE").write_text("Apache plus exception\n", encoding="utf-8")
            (crate / "COPYRIGHT").write_text("Upstream copyright\n", encoding="utf-8")
            packages = [{"name": "test-crate", "version": "1.0.0", "source": "registry+test", "license": "Apache-2.0 WITH LLVM-exception", "license_file": None, "manifest_path": str(crate / "Cargo.toml")}]
            result = collect_rust(packages, root / "legal")
            self.assertEqual(result[0]["license"], packages[0]["license"])
            self.assertEqual((root / "legal/rust/test-crate-1.0.0/COPYRIGHT").read_text(), "Upstream copyright\n")
            self.assertNotIn(str(crate), str(result))
            (crate / "LICENSE-APACHE").unlink()
            with self.assertRaises(ValueError):
                collect_rust(packages, root / "missing")

    def test_windows_uses_exact_resolved_runtime_and_preserves_notices(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            names = ["Microsoft.NETCore.App.Runtime.win-x64", "Microsoft.WindowsDesktop.App.Runtime.win-x64"]
            for name in names:
                pack = root / "nuget" / name.lower() / "8.0.99"
                pack.mkdir(parents=True)
                (pack / "LICENSE").write_text("MIT runtime\n", encoding="utf-8")
            (root / "nuget" / names[0].lower() / "8.0.99/THIRD-PARTY-NOTICES.TXT").write_text("Runtime notices\n", encoding="utf-8")
            assets = {"packageFolders": {str(root / "nuget"): {}}, "project": {"frameworks": {"net8.0-windows": {"downloadDependencies": [{"name": name, "version": "[8.0.99, 8.0.99]"} for name in names]}}}}
            result = collect_windows(assets, root / "legal")
            self.assertEqual(len(result), 2)
            self.assertTrue(all(item["version"] == "8.0.99" for item in result))
            self.assertEqual((root / "legal/dotnet" / names[0].lower() / "8.0.99/THIRD-PARTY-NOTICES.TXT").read_text(), "Runtime notices\n")
            (root / "nuget" / names[0].lower() / "8.0.99/THIRD-PARTY-NOTICES.TXT").unlink()
            with self.assertRaises(ValueError):
                collect_windows(assets, root / "incomplete")

    def test_missing_metadata_or_unsafe_package_name_is_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            for name, license in [("normal", None), ("../escape", "MIT")]:
                with self.assertRaises(ValueError):
                    collect_rust([{"name": name, "version": "1.0", "source": "registry+test", "license": license, "manifest_path": str(root / "Cargo.toml")}], root / "legal")


if __name__ == "__main__":
    unittest.main()
