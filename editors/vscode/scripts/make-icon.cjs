/**
 * Generates icons/codegrid-icon.png (128x128 RGBA) without external tools:
 * a rounded dark tile with a 3x3 cell grid (teal entries, one orange primary,
 * muted empty cells), matching the CodeGrid board metaphor.
 *
 * Run: node scripts/make-icon.cjs
 */
const zlib = require('zlib');
const fs = require('fs');
const path = require('path');

const SIZE = 128;

function crc32(buf) {
  let table = crc32.table;
  if (!table) {
    table = crc32.table = new Int32Array(256);
    for (let n = 0; n < 256; n++) {
      let c = n;
      for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
      table[n] = c;
    }
  }
  let crc = -1;
  for (let i = 0; i < buf.length; i++) crc = (crc >>> 8) ^ table[(crc ^ buf[i]) & 0xff];
  return (crc ^ -1) >>> 0;
}

function chunk(type, data) {
  const out = Buffer.alloc(8 + data.length + 4);
  out.writeUInt32BE(data.length, 0);
  out.write(type, 4, 'ascii');
  data.copy(out, 8);
  out.writeUInt32BE(crc32(out.subarray(4, 8 + data.length)), 8 + data.length);
  return out;
}

function insideRoundedRect(x, y, rx, ry, w, h, rad) {
  if (x < rx || y < ry || x >= rx + w || y >= ry + h) return false;
  const right = rx + w - 1;
  const bottom = ry + h - 1;
  const corners = [
    [rx + rad, ry + rad],
    [right - rad, ry + rad],
    [rx + rad, bottom - rad],
    [right - rad, bottom - rad],
  ];
  for (const [cx, cy] of corners) {
    const inCornerX = x < rx + rad || x > right - rad;
    const inCornerY = y < ry + rad || y > bottom - rad;
    if (inCornerX && inCornerY) {
      const dx = x - cx;
      const dy = y - cy;
      if (dx * dx + dy * dy > rad * rad) return false;
    }
  }
  return true;
}

const px = Buffer.alloc(SIZE * SIZE * 4);
function blend(x, y, r, g, b, a) {
  const i = (y * SIZE + x) * 4;
  px[i] = r;
  px[i + 1] = g;
  px[i + 2] = b;
  px[i + 3] = a;
}

const TEAL = [61, 214, 197];
const ORANGE = [255, 158, 64];
const MUTED = [90, 101, 128];
const TILE = [35, 43, 58];

// Background tile.
for (let y = 0; y < SIZE; y++) {
  for (let x = 0; x < SIZE; x++) {
    if (insideRoundedRect(x, y, 0, 0, SIZE, SIZE, 28)) {
      blend(x, y, TILE[0], TILE[1], TILE[2], 255);
    }
  }
}

// 3x3 cell grid.
const colors = [
  [TEAL, MUTED, MUTED],
  [MUTED, ORANGE, MUTED],
  [MUTED, MUTED, TEAL],
];
const cell = 26;
const gap = 9;
const margin = (SIZE - (3 * cell + 2 * gap)) / 2;
for (let row = 0; row < 3; row++) {
  for (let col = 0; col < 3; col++) {
    const cx = margin + col * (cell + gap);
    const cy = margin + row * (cell + gap);
    for (let y = 0; y < cell; y++) {
      for (let x = 0; x < cell; x++) {
        if (insideRoundedRect(cx + x, cy + y, cx, cy, cell, cell, 6)) {
          const c = colors[row][col];
          blend(cx + x, cy + y, c[0], c[1], c[2], 255);
        }
      }
    }
  }
}

// Encode PNG.
const raw = Buffer.alloc(SIZE * (SIZE * 4 + 1));
for (let y = 0; y < SIZE; y++) {
  const rowStart = y * (SIZE * 4 + 1);
  raw[rowStart] = 0; // filter: none
  px.copy(raw, rowStart + 1, y * SIZE * 4, (y + 1) * SIZE * 4);
}
const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(SIZE, 0);
ihdr.writeUInt32BE(SIZE, 4);
ihdr[8] = 8; // bit depth
ihdr[9] = 6; // color type RGBA
const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk('IHDR', ihdr),
  chunk('IDAT', zlib.deflateSync(raw, { level: 9 })),
  chunk('IEND', Buffer.alloc(0)),
]);

const out = path.join(__dirname, '..', 'icons', 'codegrid-icon.png');
fs.mkdirSync(path.dirname(out), { recursive: true });
fs.writeFileSync(out, png);
console.log(`wrote ${out} (${png.length} bytes)`);
