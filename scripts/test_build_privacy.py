"""Build privacy regressions; run with `python scripts/test_build_privacy.py`."""

import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import build_macos
from build_privacy import verify_binary_privacy
import build_windows


class RustBuildPrivacyTests(unittest.TestCase):
    def test_native_builds_preserve_encoded_flags_and_remap_paths_with_spaces(self):
        with tempfile.TemporaryDirectory(prefix="YM Link privacy ") as temporary:
            home = (Path(temporary) / "private home").resolve()
            root = home / "workspace"
            cargo = home / "custom cargo"
            rustup = home / "custom rustup"
            environment = {
                "CARGO_ENCODED_RUSTFLAGS": "-C\x1flink-arg=/already encoded/with spaces",
                "RUSTFLAGS": "--cfg ignored_by_cargo",
                "CARGO_HOME": str(cargo), "RUSTUP_HOME": str(rustup), "KEEP_ME": "present",
            }
            for module in (build_windows, build_macos):
                with self.subTest(builder=module.__name__), patch.object(module, "ROOT", root), \
                     patch.dict(os.environ, environment, clear=True), patch("pathlib.Path.home", return_value=home), \
                     patch.object(module.subprocess, "run") as command:
                    module.run(["cargo", "build"])
                    passed = command.call_args.kwargs["env"]
                    self.assertIn("CARGO_ENCODED_RUSTFLAGS", passed)
                    flags = passed["CARGO_ENCODED_RUSTFLAGS"].split("\x1f")
                    self.assertEqual(flags[:2], ["-C", "link-arg=/already encoded/with spaces"])
                    self.assertNotIn("ignored_by_cargo", flags)
                    for path, target in ((home, "home"), (cargo, "cargo"), (rustup, "rustup"), (root, "ymlink")):
                        self.assertIn(f"--remap-path-prefix={path}=/build/{target}", flags)
                        self.assertIn(f"--remap-path-prefix={path.as_posix()}=/build/{target}", flags)
                    self.assertEqual(passed["KEEP_ME"], "present")
                    self.assertTrue(flags[-1].endswith("=/build/ymlink"))

    def test_plain_rustflags_keep_cargo_whitespace_splitting(self):
        for inherited, expected in (({"RUSTFLAGS": "-C opt-level=1\t--cfg\nflag"}, ["-C", "opt-level=1", "--cfg", "flag"]),
                                    ({"RUSTFLAGS": "ignored", "CARGO_ENCODED_RUSTFLAGS": ""}, [])):
            with self.subTest(inherited=inherited), patch.dict(os.environ, inherited, clear=True), \
                 patch("pathlib.Path.home", return_value=Path(__file__).resolve().parent), \
                 patch.object(build_windows.subprocess, "run") as command:
                build_windows.run(["cargo", "build"])
                passed = command.call_args.kwargs["env"]
                self.assertIn("CARGO_ENCODED_RUSTFLAGS", passed)
                flags = passed["CARGO_ENCODED_RUSTFLAGS"].split("\x1f")
                self.assertEqual([flag for flag in flags if not flag.startswith("--remap-path-prefix=")], expected)

    def test_binary_gate_detects_separator_case_and_utf16_paths_across_read_boundary(self):
        with tempfile.TemporaryDirectory() as temporary:
            binary = Path(temporary) / "YM_Link.dll"
            for encoding in ("utf-8", "utf-16le"):
                with self.subTest(encoding=encoding):
                    payload = r"c:\Users\Private Fixture\workspace\src\lib.rs".encode(encoding)
                    binary.write_bytes(b"\x00" * (1024 * 1024 - 9) + payload)
                    with self.assertRaisesRegex(ValueError, "build-machine path.*YM_Link.dll") as failure:
                        verify_binary_privacy([binary], ["C:/users/private fixture/workspace"])
                    self.assertNotIn("Private Fixture", str(failure.exception))
            binary.write_bytes(b"/build/ymlink/src/lib.rs\x00D:/public-ci/src/lib.rs")
            verify_binary_privacy([binary], ["C:/users/private fixture/workspace"])

    def test_binary_gate_preserves_unicode_utf16_code_units_with_mixed_separators(self):
        with tempfile.TemporaryDirectory() as temporary:
            binary = Path(temporary) / "YM_Link.dll"
            private = "C:/users/山 fixture/workspace"
            payload = "c:\\Users/山 Fixture\\workspace/src/lib.rs".encode("utf-16le")
            binary.write_bytes(b"\x00" * (1024 * 1024 - 7) + payload)
            with self.assertRaisesRegex(ValueError, "build-machine path.*YM_Link.dll") as failure:
                verify_binary_privacy([binary], [private])
            self.assertNotIn("山", str(failure.exception))
            self.assertNotIn(private, str(failure.exception))

    def test_binary_gate_does_not_case_fold_bytes_inside_unicode_utf16_characters(self):
        with tempfile.TemporaryDirectory() as temporary:
            binary = Path(temporary) / "YM_Link.dll"
            # 中 (2d 4e) and 渭 (2d 6e) are distinct UTF-16LE characters.
            binary.write_bytes("C:/users/渭/workspace/src/lib.rs".encode("utf-16le"))
            verify_binary_privacy([binary], ["C:/users/中/workspace"])


if __name__ == "__main__":
    unittest.main()
