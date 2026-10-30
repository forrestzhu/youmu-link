# Development preview downloads and packaging

YM_Link currently targets **Windows x64** and **macOS on Apple Silicon**. The build target is Windows 10 1903+ / macOS 14+; the systems actually tested, and outstanding device acceptance, are recorded in [STATUS](../STATUS.md). Intel Macs and Windows ARM64 do not have a package in this pipeline.

## CI outputs

[Repository checks](../../.github/workflows/repository-checks.yml) run on pushes to `main`, pull requests, and manual dispatch. Linux runs the repository and Rust gates. Each native runner builds the application, collects exact dependency licenses, and creates:

| Platform | Installable preview | Portable / archive copy |
| --- | --- | --- |
| Windows x64 | `YM_Link-{version}-windows-x64-{sha12}-setup.exe` | `YM_Link-{version}-windows-x64-{sha12}.zip` |
| macOS arm64 | `YM_Link-{version}-macos-arm64-{sha12}.dmg` | `YM_Link-{version}-macos-arm64-{sha12}.zip` |

Every file has a sibling `.sha256` checksum. `BUILD-INFO.json` records the full source revision and platform. All packages include the project license, notice, and collected third-party license texts. The DMG also embeds them in the app's `Contents/Resources/legal`, so they remain available after installation.

Windows uses the Inno Setup 6 compiler supplied by the runner and installs to the current user's Local Application Data under `Programs/Youmu/YM_Link`. It adds a Start menu shortcut and an entry in Installed Apps; it does not request elevation. Uninstallation removes the installed files and preserves application settings and pairing state. The pipeline installs into a temporary directory, compares every installed payload byte, runs the app's bounded native backend smoke test, then uninstalls it. The smoke test refuses to overwrite a pre-existing preview installation.

The macOS DMG is a compressed, read-only disk image with an Applications shortcut. Drag `YM_Link.app` into Applications, then eject the image. Packaging verifies the image, mounts it read-only, checks its app payload and legal metadata, and verifies the signature. CI also copies the app out of the mounted image and checks localized resources in the relocated app before detaching it.

**These are unsigned development previews.** The Windows EXE is not Authenticode signed. The macOS app is ad-hoc signed, and neither the app nor DMG is notarized. Windows SmartScreen or macOS Gatekeeper may warn or block launch; producing an installer does not establish signing, notarization, OS compatibility, or real Quest playback acceptance.

## Availability

Successful main push / manual main runs upload their artifacts for **7 days**. Download them from a successful run under [Actions](https://github.com/forrestzhu/youmu-link/actions/workflows/repository-checks.yml). GitHub Actions artifact downloads require signing in to GitHub. Pull requests verify packages but do not upload them.

The workflow has read-only repository permissions and does **not** create tags, GitHub Releases, or public download assets. Publishing an externally downloadable preview is a separate maintainer action; the website must not present a direct installer link until the corresponding public asset actually exists.

## Build locally

Build the native application first; see [development setup](DEVELOPMENT.md). Then collect the licenses and package on the same OS:

```powershell
# Windows: Inno Setup 6 is required, in addition to the normal build dependencies.
python scripts/collect_licenses.py --output dist/legal --windows-assets windows/YM_Link/obj/project.assets.json
python scripts/package_installers.py --platform windows-x64
# Pass --iscc C:\path\to\ISCC.exe if the compiler is outside its standard location.
```

```bash
# macOS: uses the system codesign and hdiutil tools.
python3 scripts/collect_licenses.py --output dist/legal
python3 scripts/package_installers.py --platform macos-arm64 --smoke-test
```

Outputs are ignored under `dist/packages/`. `--revision` accepts a full 40-character source SHA and defaults to HEAD. Both installers consume exactly the ZIP payload admitted by the existing file, license, symlink / reparse-point, and build-path privacy checks. ZIP packaging remains independently available through `scripts/package_build.py`.

Use Windows `--smoke-test` on a disposable test account or CI runner: it installs, starts, and uninstalls the application and therefore writes the test account's normal settings / identity state. It refuses an existing installation, but it is not a state isolation mechanism.

## Tool references

- [GitHub Windows 2025 runner inventory](https://github.com/actions/runner-images/blob/main/images/windows/Windows2025-Readme.md)
- [Inno Setup non-administrative installation](https://jrsoftware.org/ishelp/topic_setup_privilegesrequired.htm)
- [Inno Setup installer options](https://jrsoftware.org/ishelp/topic_setupcmdline.htm) and [uninstaller options](https://jrsoftware.org/ishelp/topic_uninstcmdline.htm)
- [Apple disk image formats](https://support.apple.com/guide/disk-utility/dskutl11888/mac)
