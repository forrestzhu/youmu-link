# YM_Link app icon

The approved linked variant preserves the original Youmu solid M, negative-space Y and gold cinema horizon. Navy and gold chain links cross the lower Y, with a generous ivory clearance around them. Selected by the user on 2026-10-30 in YML-012, replacing the earlier carved variant; the editable vector is the source of truth.

- `ym-link.svg`: canonical 1024 × 1024 vector, with a transparent margin around the application tile.
- `ym-link-1024.png`: rendered master.
- `YM_Link.icns`: macOS bundle icon, including Retina representations.
- `YM_Link.ico`: Windows executable / window / installer icon at 16, 24, 32, 48, 64, 128 and 256 pixels.

To regenerate, install `sharp` in your development environment, then run:

```sh
node scripts/generate_app_icons.cjs
```

For a separately installed module, pass `--sharp-module /absolute/path/to/sharp`. Builds consume the committed exports and do not require Node.js. Copy this SVG when synchronizing the YM_Link download page; preserve the parent Youmu logo separately.
