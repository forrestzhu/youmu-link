"""Remove local Rust source prefixes and reject them in our packaged binaries."""

import os
from pathlib import Path
import re


def build_path_mappings(root, environ=None):
    environment = os.environ if environ is None else environ
    root = Path(root)
    home = Path.home()
    # rustc uses the last matching prefix. Keep the workspace last when it is
    # inside a home/cache directory, and cover both Windows separator styles.
    paths = ((home, "/build/home"),
             (root / (environment.get("CARGO_HOME") or home / ".cargo"), "/build/cargo"),
             (root / (environment.get("RUSTUP_HOME") or home / ".rustup"), "/build/rustup"),
             (root, "/build/ymlink"))
    return [(prefix, target) for path, target in paths
            for prefix in dict.fromkeys((str(path), path.as_posix(), str(path.resolve()), path.resolve().as_posix()))]


def rust_build_environment(root, environ=None):
    environment = dict(os.environ if environ is None else environ)
    # Cargo's encoded variable takes precedence even when empty. RUSTFLAGS uses
    # whitespace splitting, not shell quoting. These controlled native builds
    # take precedence over Cargo config rustflags; pass extra flags via the env.
    inherited = environment.get("CARGO_ENCODED_RUSTFLAGS")
    if inherited is None:
        inherited = "\x1f".join(environment.get("RUSTFLAGS", "").split())
    remaps = "\x1f".join(f"--remap-path-prefix={source}={target}"
                         for source, target in build_path_mappings(root, environment))
    environment["CARGO_ENCODED_RUSTFLAGS"] = inherited + ("\x1f" if inherited else "") + remaps
    return environment


def verify_binary_privacy(binaries, forbidden_paths):
    # Scan only our own executables/libraries. Public vendor build paths and
    # license examples are not evidence of a local user's private data.
    needles = []
    overlap = 0
    for path in forbidden_paths:
        prefix = str(path).rstrip("/\\")
        if not prefix:
            continue
        for encoding in ("utf-8", "utf-16le"):
            parts = []
            for character in prefix:
                # Match complete encoded characters; byte replacement/casing
                # can corrupt Unicode UTF-16LE code units (for example, 山).
                variants = "/\\" if character in "/\\" else (
                    character.lower() + character.upper() if character.isascii() else character)
                options = [re.escape(value.encode(encoding)) for value in dict.fromkeys(variants)]
                parts.append(b"(?:" + b"|".join(options) + b")" if len(options) > 1 else options[0])
            needles.append(re.compile(b"".join(parts)))
            overlap = max(overlap, len(prefix.encode(encoding)) - 1)
    for binary in binaries:
        with binary.open("rb") as stream:
            tail = b""
            for block in iter(lambda: stream.read(1024 * 1024), b""):
                chunk = tail + block
                if any(needle.search(chunk) for needle in needles):
                    raise ValueError(f"build-machine path remains in {binary.name}; rebuild before packaging")
                tail = chunk[-overlap:] if overlap else b""
