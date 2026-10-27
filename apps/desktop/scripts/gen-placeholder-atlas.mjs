// Generates the PLACEHOLDER sprite atlas (src/sprites/placeholder/) in the
// Aseprite "array" JSON format, with one frame tag per animation in
// docs/animation-spec.md. It is deliberately abstract (a grey-blue block with
// a per-animation marker) and not the buddy character: the real art is #19
// and must stay original (ADR-0005).
//
// Usage: node scripts/gen-placeholder-atlas.mjs
import { writeFileSync } from "node:fs";
import { deflateSync } from "node:zlib";
import { fileURLToPath } from "node:url";

const SIZE = 32;
/** [tag, frame count, ms per frame, marker colour] in spec order. */
const ANIMATIONS = [
  ["spawn", 8, 100, [255, 255, 255]],
  ["idle", 4, 250, [120, 200, 120]],
  ["idle_look", 6, 150, [160, 230, 160]],
  ["idle_stretch", 8, 120, [160, 230, 160]],
  ["idle_scratch", 6, 120, [160, 230, 160]],
  ["sit_down", 4, 100, [200, 200, 120]],
  ["work_type", 6, 120, [90, 160, 255]],
  ["research_read", 6, 160, [240, 240, 240]],
  ["stand_up", 4, 100, [200, 200, 120]],
  ["wait_wave", 6, 120, [255, 90, 90]],
  ["sleep", 4, 400, [150, 120, 220]],
  ["walk", 6, 100, [120, 200, 200]],
  ["grabbed", 4, 100, [255, 170, 60]],
  ["fly", 2, 80, [255, 170, 60]],
  ["land", 5, 80, [255, 170, 60]],
  ["land_splat", 4, 100, [255, 120, 40]],
  ["dizzy", 6, 120, [255, 230, 90]],
  ["leave", 8, 100, [180, 180, 180]],
];

const frames = [];
const frameTags = [];
for (const [name, count, duration, marker] of ANIMATIONS) {
  const from = frames.length;
  for (let i = 0; i < count; i++) frames.push({ name, index: i, count, duration, marker });
  frameTags.push({ name, from, to: frames.length - 1, direction: "forward" });
}

const columns = 8;
const rows = Math.ceil(frames.length / columns);
const width = columns * SIZE;
const height = rows * SIZE;
const pixels = new Uint8Array(width * height * 4); // RGBA, transparent

function put(x, y, [r, g, b], a = 255) {
  if (x < 0 || y < 0 || x >= width || y >= height) return;
  const i = (y * width + x) * 4;
  pixels.set([r, g, b, a], i);
}

frames.forEach((frame, n) => {
  const ox = (n % columns) * SIZE;
  const oy = Math.floor(n / columns) * SIZE;
  // A gentle 2 px bob so motion is visible.
  const bob = Math.round(Math.sin((frame.index / frame.count) * Math.PI * 2) * 2);
  const body = [70, 90, 120];
  const outline = [30, 40, 55];
  for (let y = 8; y < 28; y++) {
    for (let x = 6; x < 26; x++) {
      const edge = x === 6 || x === 25 || y === 8 || y === 27;
      put(ox + x, oy + y + bob, edge ? outline : body);
    }
  }
  // Marker bar: its length shows the frame index, its colour the animation.
  const bar = Math.round(((frame.index + 1) / frame.count) * 14);
  for (let x = 0; x < bar; x++) {
    put(ox + 9 + x, oy + 12 + bob, frame.marker);
    put(ox + 9 + x, oy + 13 + bob, frame.marker);
  }
});

function png(w, h, rgba) {
  const crcTable = Array.from({ length: 256 }, (_, n) => {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    return c >>> 0;
  });
  const crc = (buf) => {
    let c = 0xffffffff;
    for (const b of buf) c = crcTable[(c ^ b) & 0xff] ^ (c >>> 8);
    return (c ^ 0xffffffff) >>> 0;
  };
  const chunk = (type, data) => {
    const len = Buffer.alloc(4);
    len.writeUInt32BE(data.length);
    const body = Buffer.concat([Buffer.from(type), data]);
    const sum = Buffer.alloc(4);
    sum.writeUInt32BE(crc(body));
    return Buffer.concat([len, body, sum]);
  };
  const header = Buffer.alloc(13);
  header.writeUInt32BE(w, 0);
  header.writeUInt32BE(h, 4);
  header.set([8, 6, 0, 0, 0], 8); // 8-bit RGBA
  const raw = Buffer.alloc((w * 4 + 1) * h);
  for (let y = 0; y < h; y++) {
    raw[y * (w * 4 + 1)] = 0;
    Buffer.from(rgba.buffer, y * w * 4, w * 4).copy(raw, y * (w * 4 + 1) + 1);
  }
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", header),
    chunk("IDAT", deflateSync(raw, { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

const atlas = {
  frames: frames.map((frame, n) => ({
    filename: `${frame.name} ${frame.index}`,
    frame: { x: (n % columns) * SIZE, y: Math.floor(n / columns) * SIZE, w: SIZE, h: SIZE },
    rotated: false,
    trimmed: false,
    spriteSourceSize: { x: 0, y: 0, w: SIZE, h: SIZE },
    sourceSize: { w: SIZE, h: SIZE },
    duration: frame.duration,
  })),
  meta: {
    app: "claude-buddy placeholder generator",
    image: "atlas.png",
    format: "RGBA8888",
    size: { w: width, h: height },
    scale: "1",
    frameTags,
  },
};

const out = (file) => fileURLToPath(new URL(`../src/sprites/placeholder/${file}`, import.meta.url));
writeFileSync(out("atlas.png"), png(width, height, pixels));
writeFileSync(out("atlas.json"), JSON.stringify(atlas, null, 2) + "\n");
console.log(`placeholder atlas: ${frames.length} frames, ${frameTags.length} tags, ${width}x${height}`);
