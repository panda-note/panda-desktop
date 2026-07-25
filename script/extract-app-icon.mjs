// Run: bun add -d sharp && bun script/extract-app-icon.mjs <source.png>
import sharp from "sharp";
import { readFileSync, writeFileSync, mkdirSync } from "fs";
import { dirname, join } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));

const SOURCE = process.argv[2];
const OUT_DIR = process.argv[3] ?? join(__dirname, "../crates/zed/resources");

if (!SOURCE) {
  console.error("Usage: bun script/extract-app-icon.mjs <source.png> [out-dir]");
  process.exit(1);
}

const BLACK_THRESHOLD = 18;

function isBackgroundBlack(r, g, b) {
  return r <= BLACK_THRESHOLD && g <= BLACK_THRESHOLD && b <= BLACK_THRESHOLD;
}

async function extractIcon(inputPath) {
  const { data, info } = await sharp(inputPath)
    .ensureAlpha()
    .raw()
    .toBuffer({ resolveWithObject: true });

  const { width, height, channels } = info;
  const pixels = new Uint8Array(data);
  const visited = new Uint8Array(width * height);
  const transparent = new Uint8Array(width * height);
  const queue = [];

  const idx = (x, y) => y * width + x;

  for (let x = 0; x < width; x++) {
    for (const y of [0, height - 1]) {
      const i = idx(x, y);
      const o = i * channels;
      if (!visited[i] && isBackgroundBlack(pixels[o], pixels[o + 1], pixels[o + 2])) {
        visited[i] = 1;
        transparent[i] = 1;
        queue.push(i);
      }
    }
  }
  for (let y = 0; y < height; y++) {
    for (const x of [0, width - 1]) {
      const i = idx(x, y);
      const o = i * channels;
      if (!visited[i] && isBackgroundBlack(pixels[o], pixels[o + 1], pixels[o + 2])) {
        visited[i] = 1;
        transparent[i] = 1;
        queue.push(i);
      }
    }
  }

  while (queue.length > 0) {
    const i = queue.pop();
    const x = i % width;
    const y = (i - x) / width;
    for (const [dx, dy] of [
      [1, 0],
      [-1, 0],
      [0, 1],
      [0, -1],
    ]) {
      const nx = x + dx;
      const ny = y + dy;
      if (nx < 0 || ny < 0 || nx >= width || ny >= height) continue;
      const ni = idx(nx, ny);
      if (visited[ni]) continue;
      const o = ni * channels;
      if (!isBackgroundBlack(pixels[o], pixels[o + 1], pixels[o + 2])) continue;
      visited[ni] = 1;
      transparent[ni] = 1;
      queue.push(ni);
    }
  }

  let minX = width;
  let minY = height;
  let maxX = 0;
  let maxY = 0;

  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const i = idx(x, y);
      if (transparent[i]) {
        pixels[i * channels + 3] = 0;
        continue;
      }
      if (x < minX) minX = x;
      if (y < minY) minY = y;
      if (x > maxX) maxX = x;
      if (y > maxY) maxY = y;
    }
  }

  const cropW = maxX - minX + 1;
  const cropH = maxY - minY + 1;
  const cropped = Buffer.alloc(cropW * cropH * channels);

  for (let y = 0; y < cropH; y++) {
    for (let x = 0; x < cropW; x++) {
      const src = idx(minX + x, minY + y) * channels;
      const dst = (y * cropW + x) * channels;
      cropped[dst] = pixels[src];
      cropped[dst + 1] = pixels[src + 1];
      cropped[dst + 2] = pixels[src + 2];
      cropped[dst + 3] = pixels[src + 3];
    }
  }

  return sharp(cropped, { raw: { width: cropW, height: cropH, channels } }).png();
}

async function writeSized(base, size, path) {
  await base.clone().resize(size, size, { fit: "contain", background: { r: 0, g: 0, b: 0, alpha: 0 } }).png().toFile(path);
  console.log(`wrote ${path} (${size}x${size})`);
}

mkdirSync(OUT_DIR, { recursive: true });
mkdirSync(join(OUT_DIR, "windows"), { recursive: true });

const extracted = await extractIcon(SOURCE);

const previewPath = join(OUT_DIR, "_extracted-preview.png");
await extracted.clone().png().toFile(previewPath);
console.log(`preview: ${previewPath}`);

const targets = [
  ["app-icon.png", 256],
  ["app-icon@2x.png", 512],
  ["app-icon-dev.png", 256],
  ["app-icon-dev@2x.png", 512],
  ["app-icon-preview.png", 256],
  ["app-icon-preview@2x.png", 512],
  ["app-icon-nightly.png", 256],
  ["app-icon-nightly@2x.png", 512],
];

for (const [name, size] of targets) {
  await writeSized(extracted, size, join(OUT_DIR, name));
}

// Windows ICO: embed common sizes
const icoSizes = [16, 24, 32, 48, 64, 128, 256];
const pngBuffers = await Promise.all(
  icoSizes.map((size) =>
    extracted
      .clone()
      .resize(size, size, { fit: "contain", background: { r: 0, g: 0, b: 0, alpha: 0 } })
      .png()
      .toBuffer(),
  ),
);

// Minimal ICO writer (PNG-compressed entries, Vista+)
function writeIco(pngs, sizes) {
  const count = pngs.length;
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(count, 4);

  const dirEntries = [];
  let offset = 6 + count * 16;
  for (let i = 0; i < count; i++) {
    const size = sizes[i];
    const png = pngs[i];
    const entry = Buffer.alloc(16);
    entry.writeUInt8(size >= 256 ? 0 : size, 0);
    entry.writeUInt8(size >= 256 ? 0 : size, 1);
    entry.writeUInt8(0, 2);
    entry.writeUInt8(0, 3);
    entry.writeUInt16LE(1, 4);
    entry.writeUInt16LE(32, 6);
    entry.writeUInt32LE(png.length, 8);
    entry.writeUInt32LE(offset, 12);
    dirEntries.push(entry);
    offset += png.length;
  }

  return Buffer.concat([header, ...dirEntries, ...pngs]);
}

const ico = writeIco(pngBuffers, icoSizes);
const icoNames = [
  "windows/app-icon.ico",
  "windows/app-icon-dev.ico",
  "windows/app-icon-preview.ico",
  "windows/app-icon-nightly.ico",
];

for (const name of icoNames) {
  const path = join(OUT_DIR, name);
  writeFileSync(path, ico);
  console.log(`wrote ${path}`);
}

// auto_update_helper icon
writeFileSync(join(__dirname, "../crates/auto_update_helper/app-icon.ico"), ico);
console.log("wrote crates/auto_update_helper/app-icon.ico");
