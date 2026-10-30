/* Regenerate checked-in app-icon assets from their editable SVG source. */
const fs = require('node:fs/promises');
const path = require('node:path');

const args = process.argv.slice(2);
if (args.length && (args.length !== 2 || args[0] !== '--sharp-module')) {
  throw new Error('Usage: node scripts/generate_app_icons.cjs [--sharp-module <installed sharp path>]');
}
const sharp = require(args[1] || 'sharp');
const folder = path.resolve(__dirname, '../assets/app-icon');
const sizes = [16, 24, 32, 48, 64, 128, 256, 512, 1024];

function icns(images) {
  // PNG representations supported by the macOS 14+ deployment target.
  const types = [[16, 'icp4'], [32, 'icp5'], [64, 'icp6'], [128, 'ic07'],
    [256, 'ic08'], [512, 'ic09'], [1024, 'ic10'], [32, 'ic11'],
    [64, 'ic12'], [256, 'ic13'], [512, 'ic14']];
  const records = types.map(([size, type]) => {
    const body = images.get(size);
    const header = Buffer.alloc(8);
    header.write(type); header.writeUInt32BE(body.length + 8, 4);
    return Buffer.concat([header, body]);
  });
  const header = Buffer.alloc(8);
  header.write('icns'); header.writeUInt32BE(8 + records.reduce((n, record) => n + record.length, 0), 4);
  return Buffer.concat([header, ...records]);
}

function ico(images) {
  const variants = sizes.filter(size => size <= 256);
  const header = Buffer.alloc(6);
  header.writeUInt16LE(1, 2); header.writeUInt16LE(variants.length, 4);
  let offset = 6 + variants.length * 16;
  const entries = variants.map(size => {
    const body = images.get(size), entry = Buffer.alloc(16);
    entry[0] = entry[1] = size === 256 ? 0 : size;
    entry.writeUInt16LE(1, 4); entry.writeUInt16LE(32, 6);
    entry.writeUInt32LE(body.length, 8); entry.writeUInt32LE(offset, 12);
    offset += body.length;
    return entry;
  });
  return Buffer.concat([header, ...entries, ...variants.map(size => images.get(size))]);
}

(async () => {
  const source = await fs.readFile(path.join(folder, 'ym-link.svg'));
  const master = await sharp(source).toColourspace('srgb').png().toBuffer();
  const images = new Map();
  for (const size of sizes) {
    images.set(size, await sharp(master).resize(size, size).png().toBuffer());
  }
  await fs.writeFile(path.join(folder, 'ym-link-1024.png'), master);
  await fs.writeFile(path.join(folder, 'YM_Link.icns'), icns(images));
  await fs.writeFile(path.join(folder, 'YM_Link.ico'), ico(images));
  console.log(`Generated PNG, ICNS and ICO from ym-link.svg (sharp ${sharp.versions.sharp}).`);
})().catch(error => { console.error(error.message); process.exitCode = 1; });
