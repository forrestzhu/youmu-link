# Third-party software

YM_Link's original code is licensed under Apache-2.0. This does not replace the licenses of its dependencies.

The Rust dependency graph is pinned by Cargo.lock. Developer packages contain full upstream license, copyright, and notice files in `licenses/rust/`, together with an index of package names, versions, and declared SPDX expressions. This inventory includes the locked build, test, and platform-specific dependency graph and may include packages not linked into a particular binary. Preserve these files when redistributing a package.

The graph includes MIT, Apache-2.0, BSD-3-Clause, Unicode-3.0, and Apache-2.0 WITH LLVM-exception terms, including the BSD notices for curve25519-dalek, subtle, and x25519-dalek, the LLVM exception for winx, and Unicode terms for unicode-ident. The generated bundle preserves all supplied alternatives and additions; it does not relicense these components.

The Rust standard library and runtime are statically embedded and are not part of this project's Cargo.lock graph. Their LICENSE-APACHE, LICENSE-MIT, COPYRIGHT, and any additional supplied license files are preserved from the exact rustc sysroot under `licenses/rust-toolchain/`, with the Rust version in the generated index. These include separate bundled third-party notices; they are not replaced by the project's Apache license.

Windows self-contained builds also bundle Microsoft .NET and Windows Desktop runtime components. Their MIT license files and the .NET runtime's third-party notices are copied from the exact restored runtime packs into `licenses/dotnet/`. Their versions are taken from the build's project.assets.json, rather than the development machine's installed version.

The macOS Swift package has no remote package dependencies; the Windows WPF projects have no PackageReference dependencies. Apple and Windows system frameworks are linked through their platform SDKs and are not redistributed as part of these developer packages.

To regenerate the legal bundle after building:

```text
python scripts/collect_licenses.py --output dist/legal
python scripts/collect_licenses.py --output dist/legal --windows-assets windows/YM_Link/obj/project.assets.json
```

The collector fails if a third-party package has no license metadata or no discoverable license text. It collects notices; it is not a substitute for reviewing new dependency licenses. No original source from the architectural reference project was incorporated into YM_Link.
