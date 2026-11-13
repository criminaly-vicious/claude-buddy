// Draws the buddy, Clawd, pixel by pixel and writes everything made from it:
//
//   src/sprites/buddy/atlas.png + atlas.json   the overlay's sprite atlas
//                                              (Aseprite "array" JSON, one
//                                              frame tag per animation in
//                                              docs/animation-spec.md)
//   docs/img/buddy/clips.png                   every clip, frame by frame
//   docs/img/buddy/demo.gif                    a short animated tour
//   src-tauri/icons/*                          the app and installer icons
//
// The art is code, not an image file: a block-shaped body with a darker
// outline, two slit eyes, arm nubs and four short legs, plus props drawn as
// separate layers (laptop, paper sheet, "!" bubble, "z", sweat drop, dizzy
// stars, dust). Each frame is a pure function of the clip and frame index,
// so running the script twice gives identical files.
//
// Usage: node scripts/gen-buddy-atlas.mjs (from apps/desktop)
import { mkdirSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { gif } from "./gif.mjs";
import { png } from "./png.mjs";

const SIZE = 32;
/** First empty row under the feet. */
const FLOOR = 30;

// --- Palette ------------------------------------------------------------------

const hex = (h) => [parseInt(h.slice(1, 3), 16), parseInt(h.slice(3, 5), 16), parseInt(h.slice(5, 7), 16)];
const C = {
  body: hex("#D97757"),
  shade: hex("#BF6447"),
  outline: hex("#7A3523"),
  eye: hex("#1F1411"),
  glow: hex("#BFE8FF"),
  metal: hex("#4A505C"),
  metalLight: hex("#7C8494"),
  metalDark: hex("#2C3038"),
  paper: hex("#FFFFFF"),
  paperLine: hex("#A9B4C2"),
  ink: hex("#1F1411"),
  alert: hex("#E5484D"),
  zzz: hex("#8FA8FF"),
  sweat: hex("#7CC6F2"),
  star: hex("#FFD54A"),
  dust: hex("#D8CFC4"),
};

// --- Layers ---------------------------------------------------------------------

/** A 32x32 layer of colours (null = transparent). */
class Layer {
  constructor() {
    this.px = new Array(SIZE * SIZE).fill(null);
  }
  set(x, y, c) {
    if (x >= 0 && y >= 0 && x < SIZE && y < SIZE) this.px[y * SIZE + x] = c;
  }
  get(x, y) {
    return x >= 0 && y >= 0 && x < SIZE && y < SIZE ? this.px[y * SIZE + x] : null;
  }
  /** Draws rows of characters, each mapped to a colour; "." is skipped. */
  stamp(x0, y0, rows, colours) {
    rows.forEach((row, dy) =>
      [...row].forEach((ch, dx) => {
        if (colours[ch]) this.set(x0 + dx, y0 + dy, colours[ch]);
      }),
    );
  }
  /** Copies this layer's pixels over `base`. */
  over(base) {
    this.px.forEach((c, i) => {
      if (c) base.px[i] = c;
    });
    return base;
  }
  /** Rotated a quarter turn clockwise around the centre. */
  rotated() {
    const out = new Layer();
    for (let y = 0; y < SIZE; y++) {
      for (let x = 0; x < SIZE; x++) out.set(SIZE - 1 - y, x, this.get(x, y));
    }
    return out;
  }
  shifted(dx, dy) {
    const out = new Layer();
    for (let y = 0; y < SIZE; y++) {
      for (let x = 0; x < SIZE; x++) out.set(x + dx, y + dy, this.get(x, y));
    }
    return out;
  }
}

/**
 * A solid shape (body, limbs) built from rectangles, painted with the body
 * colour, a shade along its bottom edge and a 1 px outline around it all.
 */
class Shape {
  constructor() {
    this.mask = new Uint8Array(SIZE * SIZE);
  }
  rect(x0, y0, w, h) {
    for (let y = y0; y < y0 + h; y++) {
      for (let x = x0; x < x0 + w; x++) {
        if (x >= 0 && y >= 0 && x < SIZE && y < SIZE) this.mask[y * SIZE + x] = 1;
      }
    }
    return this;
  }
  has(x, y) {
    return x >= 0 && y >= 0 && x < SIZE && y < SIZE && this.mask[y * SIZE + x] === 1;
  }
  paint() {
    const layer = new Layer();
    for (let y = 0; y < SIZE; y++) {
      for (let x = 0; x < SIZE; x++) {
        if (this.has(x, y)) {
          layer.set(x, y, this.has(x, y + 1) ? C.body : C.shade);
        } else if (this.has(x + 1, y) || this.has(x - 1, y) || this.has(x, y + 1) || this.has(x, y - 1)) {
          layer.set(x, y, C.outline);
        }
      }
    }
    return layer;
  }
}

// --- Clawd ----------------------------------------------------------------------

/**
 * Draws Clawd. Options:
 * - w, h: body size (20 × 12 at rest; squash and stretch change it)
 * - dx, dy: offset; the body otherwise stands on the floor, centred
 * - pose: "stand" (legs), "sit" (legs folded under), "air" (legs dangle)
 * - legs: per-leg lift in px for walking/kicking, [l1, l2, r1, r2]
 * - arms: { left, right } each "down", "up", "out", "wave", "head", "front", "none"
 * - eyes: "open", "closed", "down", "left", "right", "wide", "x"
 */
function clawd(o = {}) {
  const w = o.w ?? 20;
  const h = o.h ?? 12;
  const pose = o.pose ?? "stand";
  const legH = pose === "stand" ? 3 : 0;
  const bx = Math.round((SIZE - w) / 2) + (o.dx ?? 0);
  const by = FLOOR - legH - h + (o.dy ?? 0) - (pose === "air" ? 4 : 0);
  const shape = new Shape().rect(bx, by, w, h);

  // Four short legs: two under each side.
  const legX = [bx + 2, bx + 6, bx + w - 8, bx + w - 4];
  const lift = o.legs ?? [0, 0, 0, 0];
  if (pose === "stand") {
    legX.forEach((x, i) => shape.rect(x, by + h, 2, Math.max(1, legH - lift[i])));
  } else if (pose === "air") {
    legX.forEach((x, i) => shape.rect(x + (lift[i] > 0 ? 1 : 0), by + h, 2, 3 - (lift[i] > 1 ? 1 : 0)));
  }

  // Arm nubs on the sides, a third of the way down.
  const armY = by + Math.max(2, Math.round(h / 3));
  const arms = { left: "down", right: "down", ...(o.arms ?? {}) };
  const arm = (side, kind) => {
    const outer = side === "left" ? bx - 3 : bx + w;
    const sign = side === "left" ? -1 : 1;
    switch (kind) {
      case "down":
        shape.rect(outer, armY, 3, 3);
        break;
      case "out":
        shape.rect(outer + sign, armY - 1, 3, 2);
        break;
      case "up":
        shape.rect(side === "left" ? bx - 2 : bx + w, by - 4, 2, 6);
        break;
      case "wave":
        // Raised and tilted outwards.
        shape.rect(side === "left" ? bx - 3 : bx + w, by - 1, 2, 4);
        shape.rect(side === "left" ? bx - 4 : bx + w + 1, by - 4, 2, 4);
        break;
      case "wave2":
        shape.rect(side === "left" ? bx - 2 : bx + w, by - 5, 2, 7);
        break;
      case "head":
        // Hand on top of the head, scratching.
        shape.rect(side === "left" ? bx - 2 : bx + w, by - 2, 2, 4);
        shape.rect(side === "left" ? bx : bx + w - 4, by - 3, 4, 2);
        break;
      default:
        break;
    }
  };
  arm("left", arms.left);
  arm("right", arms.right);

  const layer = shape.paint();

  // Two slit eyes near the top.
  const eyeH = Math.max(1, Math.min(4, h - 6));
  const ey = by + Math.max(2, Math.round(h / 4));
  const lx = bx + Math.round(w * 0.27);
  const rx = bx + w - Math.round(w * 0.27) - 2;
  const eyes = o.eyes ?? "open";
  const slit = (x, y, hh) => {
    for (let i = 0; i < hh; i++) {
      layer.set(x, y + i, C.eye);
      layer.set(x + 1, y + i, C.eye);
    }
  };
  for (const x of [lx, rx]) {
    switch (eyes) {
      case "closed":
        layer.set(x - 1, ey + eyeH - 1, C.eye);
        layer.set(x, ey + eyeH - 1, C.eye);
        layer.set(x + 1, ey + eyeH - 1, C.eye);
        layer.set(x + 2, ey + eyeH - 1, C.eye);
        break;
      case "down":
        slit(x, ey + 1, 2);
        break;
      case "left":
        slit(x - 1, ey, eyeH);
        break;
      case "right":
        slit(x + 1, ey, eyeH);
        break;
      case "wide":
        slit(x, ey - 1, eyeH + 1);
        break;
      case "x":
        layer.set(x - 1, ey, C.eye);
        layer.set(x + 2, ey, C.eye);
        layer.set(x, ey + 1, C.eye);
        layer.set(x + 1, ey + 1, C.eye);
        layer.set(x - 1, ey + 2, C.eye);
        layer.set(x + 2, ey + 2, C.eye);
        break;
      default:
        slit(x, ey, eyeH);
    }
  }
  return { layer, box: { x: bx, y: by, w, h }, eye: { lx, rx, y: ey } };
}

// --- Props ------------------------------------------------------------------------

/** The laptop lid's back, below the eyes of a sitting Clawd; `glowOn` lights the face. */
function laptop(layer, glowOn) {
  layer.stamp(9, 25, [
    ".kkkkkkkkkkkk.",
    "kmMMMMMMMMMMmk",
    "kmmmmmmmmmmmmk",
    "kkkkkkkkkkkkkk",
    "kmmmmmmmmmmmmk",
  ], { k: C.metalDark, m: C.metalLight, M: C.metal });
  if (glowOn) {
    // Screen light on the cheeks.
    for (const x of [10, 11, 20, 21]) layer.set(x, 24, C.glow);
  }
}

/** Hands on the keyboard edge: which one is pressing (0 or 1). */
function typingHands(layer, press) {
  const hand = (x, down) => new Shape().rect(x, down ? 28 : 27, 2, 2).paint().over(layer);
  hand(6, press === 0);
  hand(24, press === 1);
}

/** A sheet of paper held below the eyes; `flip` shows it mid page-turn. */
function paper(layer, flip) {
  const rows = flip
    ? ["..kkkkkk", ".kppppk.", "kplllpk.", "kpppppk.", "kkkkkkk."]
    : ["kkkkkkkkkk", "kpllllllpk", "kppppppppk", "kplllllppk", "kkkkkkkkkk"];
  layer.stamp(11, 25, rows, { k: C.ink, p: C.paper, l: C.paperLine });
  // Hands holding the sheet's sides.
  new Shape().rect(8, 26, 2, 2).paint().over(layer);
  new Shape().rect(22, 26, 2, 2).paint().over(layer);
}

/** Speech bubble with a red "!", its tail pointing down towards the head. */
function alertBubble(layer, x, y) {
  layer.stamp(x, y, [
    ".kkkkk.",
    "kpprppk",
    "kpprppk",
    "kpprppk",
    "kpppppk",
    "kpprppk",
    ".kkkkpk",
    "....kk.",
  ], { k: C.ink, p: C.paper, r: C.alert });
}

const Z_BIG = ["zzzzz", "...z.", "..z..", ".z...", "zzzzz"];
const Z_SMALL = ["zzzz", "..z.", ".z..", "zzzz"];

/** Rising "z"s: `t` in 0..3 moves them up and fades the top one out. */
function zees(layer, t) {
  layer.stamp(21, 12 - t, Z_BIG, { z: C.zzz });
  if (t < 3) layer.stamp(27, 6 - t, Z_SMALL, { z: C.zzz });
}

function sweat(layer, x, y) {
  layer.stamp(x, y, [".s", "ss", "ss"], { s: C.sweat });
}

/** Three stars circling above the head, at step `t`. */
function stars(layer, cx, cy, t) {
  for (let i = 0; i < 3; i++) {
    const a = ((t * 60 + i * 120) * Math.PI) / 180;
    const x = Math.round(cx + Math.cos(a) * 9);
    const y = Math.round(cy + Math.sin(a) * 3);
    layer.stamp(x - 1, y - 1, [".s.", "sss", ".s."], { s: C.star });
  }
}

/** Dust puffs at the feet, spreading as `t` grows (0..3). */
function dust(layer, t) {
  const spread = 4 + t * 3;
  for (const side of [-1, 1]) {
    const x = 16 + side * spread;
    const size = t < 3 ? 2 : 1;
    for (let dy = 0; dy < size; dy++) {
      for (let dx = 0; dx < size + 1; dx++) layer.set(x + dx - 1, FLOOR - 1 - dy, C.dust);
    }
  }
}

// --- Clips --------------------------------------------------------------------------

const walkLegs = [
  [1, 0, 1, 0],
  [2, 0, 2, 0],
  [1, 0, 1, 0],
  [0, 1, 0, 1],
  [0, 2, 0, 2],
  [0, 1, 0, 1],
];

/** [tag, ms per frame, frames]; each frame returns a Layer. Spec order. */
const CLIPS = [
  [
    "spawn",
    100,
    [-24, -16, -8, 0, 0, -3, -1, 0].map((dy, i) => () => {
      const squash = i === 3 ? { w: 22, h: 10 } : i === 4 ? { w: 21, h: 11 } : {};
      const base = new Layer();
      if (i >= 3 && i <= 6) dust(base, i - 3);
      if (i === 0) dust(base, 0);
      const air = dy < 0;
      return clawd({ ...squash, dy, legs: air ? [1, 1, 1, 1] : undefined, arms: air ? { left: "up", right: "up" } : {} }).layer.over(base);
    }),
  ],
  ["idle", 250, [0, 0, 1, 1].map((b) => () => clawd({ h: 12 - b }).layer)],
  [
    "idle_look",
    150,
    ["open", "left", "left", "right", "right", "open"].map((eyes) => () => clawd({ eyes }).layer),
  ],
  [
    "idle_stretch",
    120,
    [0, 1, 2, 3, 3, 2, 1, 0].map((s) => () =>
      clawd({
        h: 12 + Math.min(s, 2),
        w: 20 - (s >= 2 ? 1 : 0),
        arms: s >= 1 ? { left: "up", right: "up" } : {},
        eyes: s >= 2 ? "closed" : "open",
      }).layer,
    ),
  ],
  [
    "idle_scratch",
    120,
    [0, 1, 0, 1, 0, 1].map((t) => () =>
      clawd({ arms: { right: "head" }, eyes: t ? "closed" : "open", dx: t ? 0 : 0, h: 12 - t }).layer,
    ),
  ],
  [
    "sit_down",
    100,
    [0, 1, 2, 3].map((t) => () => {
      if (t === 3) return clawd({ pose: "sit" }).layer;
      return clawd({ dy: t + 1, h: 12 - (t === 2 ? 1 : 0), legs: [t, t, t, t] }).layer;
    }),
  ],
  [
    "work_type",
    120,
    [0, 1, 0, 1, 0, 1].map((press, i) => () => {
      const c = clawd({ pose: "sit", eyes: "down", arms: { left: "none", right: "none" } }).layer;
      laptop(c, i === 3 || i === 4);
      typingHands(c, press);
      return c;
    }),
  ],
  [
    "research_read",
    160,
    ["left", "left", "down", "right", "right", "flip"].map((look) => () => {
      const c = clawd({
        pose: "sit",
        eyes: look === "flip" ? "open" : look,
        arms: { left: "none", right: "none" },
      }).layer;
      paper(c, look === "flip");
      return c;
    }),
  ],
  [
    "stand_up",
    100,
    [3, 2, 1, 0].map((t) => () => {
      if (t === 3) return clawd({ pose: "sit" }).layer;
      return clawd({ dy: t + 1, h: 12 + (t === 0 ? 1 : 0), legs: [t, t, t, t] }).layer;
    }),
  ],
  [
    "wait_wave",
    120,
    [0, 1, 0, 1, 0, 1].map((t) => () => {
      const c = clawd({ arms: { right: t ? "wave2" : "wave" }, eyes: "wide" }).layer;
      alertBubble(c, 2, 2 + (t ? 1 : 0));
      return c;
    }),
  ],
  [
    "sleep",
    400,
    [0, 1, 2, 3].map((t) => () => {
      const c = clawd({ pose: "sit", eyes: "closed", h: t % 2 ? 11 : 12, w: t % 2 ? 21 : 20, arms: { left: "none", right: "none" } }).layer;
      zees(c, t);
      return c;
    }),
  ],
  ["walk", 100, walkLegs.map((legs, i) => () => clawd({ legs, dy: i % 3 === 1 ? -1 : 0 }).layer)],
  [
    "grabbed",
    100,
    [0, 1, 2, 3].map((t) => () => {
      const legs = t % 2 ? [2, 0, 2, 0] : [0, 2, 0, 2];
      const c = clawd({ pose: "air", legs, arms: { left: "up", right: "up" }, eyes: "wide" }).layer;
      if (t >= 2) sweat(c, 27, 8 + (t - 2));
      return c;
    }),
  ],
  [
    "fly",
    80,
    [0, 1].map((t) => () => {
      const c = clawd({ pose: "air", legs: [1, 0, 1, 0], arms: { left: "out", right: "out" }, eyes: "wide", dy: 2 }).layer;
      return t ? c.rotated() : c;
    }),
  ],
  [
    "land",
    80,
    [
      { w: 24, h: 8 },
      { w: 23, h: 9 },
      { w: 21, h: 11 },
      { w: 20, h: 13 },
      { w: 20, h: 12 },
    ].map((sq, i) => () => {
      const base = new Layer();
      if (i < 2) dust(base, i);
      return clawd({ ...sq, eyes: i < 2 ? "closed" : "open" }).layer.over(base);
    }),
  ],
  [
    "land_splat",
    100,
    [
      { w: 28, h: 5 },
      { w: 27, h: 6 },
      { w: 25, h: 8 },
      { w: 22, h: 10 },
    ].map((sq, i) => () => {
      const base = new Layer();
      dust(base, Math.min(3, i + 1));
      return clawd({ ...sq, pose: "sit", eyes: "x", arms: i < 2 ? { left: "none", right: "none" } : {} }).layer.over(base);
    }),
  ],
  [
    "dizzy",
    120,
    [0, 1, 2, 3, 4, 5].map((t) => () => {
      const sway = [0, -1, 0, 1, 0, 0][t];
      const done = t === 5;
      const c = clawd({ dx: sway, eyes: done ? "closed" : t % 2 ? "left" : "right" }).layer;
      if (!done) stars(c, 16 + sway, 11, t);
      return c;
    }),
  ],
  [
    "leave",
    100,
    [0, 0, 0, 4, 8, 13, 19, 32].map((dx, i) => () => {
      if (i < 3) return clawd({ arms: { right: i === 1 ? "wave2" : "wave" } }).layer;
      return clawd({ dx, legs: walkLegs[i % walkLegs.length] }).layer;
    }),
  ],
];

// --- Atlas -------------------------------------------------------------------------

const frames = [];
const frameTags = [];
for (const [name, duration, makers] of CLIPS) {
  const from = frames.length;
  makers.forEach((make, index) => frames.push({ name, index, duration, layer: make() }));
  frameTags.push({ name, from, to: frames.length - 1, direction: "forward" });
}

const COLUMNS = 8;
const atlasW = COLUMNS * SIZE;
const atlasH = Math.ceil(frames.length / COLUMNS) * SIZE;

function rgbaOf(w, h, draw) {
  const rgba = new Uint8Array(w * h * 4);
  draw((x, y, [r, g, b], a = 255) => {
    if (x >= 0 && y >= 0 && x < w && y < h) rgba.set([r, g, b, a], (y * w + x) * 4);
  });
  return rgba;
}

const atlasPng = png(
  atlasW,
  atlasH,
  rgbaOf(atlasW, atlasH, (put) =>
    frames.forEach((f, n) => {
      const ox = (n % COLUMNS) * SIZE;
      const oy = Math.floor(n / COLUMNS) * SIZE;
      for (let y = 0; y < SIZE; y++) {
        for (let x = 0; x < SIZE; x++) {
          const c = f.layer.get(x, y);
          if (c) put(ox + x, oy + y, c);
        }
      }
    }),
  ),
);

const atlasJson = {
  frames: frames.map((f, n) => ({
    filename: `${f.name} ${f.index}`,
    frame: { x: (n % COLUMNS) * SIZE, y: Math.floor(n / COLUMNS) * SIZE, w: SIZE, h: SIZE },
    rotated: false,
    trimmed: false,
    spriteSourceSize: { x: 0, y: 0, w: SIZE, h: SIZE },
    sourceSize: { w: SIZE, h: SIZE },
    duration: f.duration,
  })),
  meta: {
    app: "claude-buddy gen-buddy-atlas.mjs",
    image: "atlas.png",
    format: "RGBA8888",
    size: { w: atlasW, h: atlasH },
    scale: "1",
    frameTags,
  },
};

// --- Preview sheet: one row per clip -------------------------------------------------

const FONT = {
  a: [".#.", "#.#", "###", "#.#", "#.#"],
  b: ["##.", "#.#", "##.", "#.#", "##."],
  c: [".##", "#..", "#..", "#..", ".##"],
  d: ["##.", "#.#", "#.#", "#.#", "##."],
  e: ["###", "#..", "##.", "#..", "###"],
  f: ["###", "#..", "##.", "#..", "#.."],
  g: [".##", "#..", "#.#", "#.#", ".##"],
  h: ["#.#", "#.#", "###", "#.#", "#.#"],
  i: ["###", ".#.", ".#.", ".#.", "###"],
  k: ["#.#", "#.#", "##.", "#.#", "#.#"],
  l: ["#..", "#..", "#..", "#..", "###"],
  m: ["#.#", "###", "###", "#.#", "#.#"],
  n: ["##.", "#.#", "#.#", "#.#", "#.#"],
  o: [".#.", "#.#", "#.#", "#.#", ".#."],
  p: ["##.", "#.#", "##.", "#..", "#.."],
  r: ["##.", "#.#", "##.", "#.#", "#.#"],
  s: [".##", "#..", ".#.", "..#", "##."],
  t: ["###", ".#.", ".#.", ".#.", ".#."],
  u: ["#.#", "#.#", "#.#", "#.#", "###"],
  v: ["#.#", "#.#", "#.#", "#.#", ".#."],
  w: ["#.#", "#.#", "###", "###", "#.#"],
  y: ["#.#", "#.#", ".#.", ".#.", ".#."],
  z: ["###", "..#", ".#.", "#..", "###"],
  _: ["...", "...", "...", "...", "###"],
  " ": ["...", "...", "...", "...", "..."],
};

function clipsSheet() {
  const scale = 4;
  const cell = SIZE * scale;
  const pad = 8;
  const labelW = 14 * 4 * 2 + pad * 2;
  const maxFrames = Math.max(...CLIPS.map(([, , m]) => m.length));
  const w = labelW + maxFrames * (cell + pad) + pad;
  const h = pad + CLIPS.length * (cell + pad);
  const bg = hex("#F4F1EC");
  const panel = hex("#E8E2D9");
  return png(
    w,
    h,
    rgbaOf(w, h, (put) => {
      const fill = (x0, y0, ww, hh, c) => {
        for (let y = y0; y < y0 + hh; y++) for (let x = x0; x < x0 + ww; x++) put(x, y, c);
      };
      fill(0, 0, w, h, bg);
      let n = 0;
      CLIPS.forEach(([name, , makers], row) => {
        const y0 = pad + row * (cell + pad);
        [...name].forEach((ch, i) =>
          (FONT[ch] ?? FONT[" "]).forEach((r, gy) =>
            [...r].forEach((on, gx) => {
              if (on === "#") fill(pad + (i * 4 + gx) * 2, y0 + cell / 2 - 5 + gy * 2, 2, 2, C.ink);
            }),
          ),
        );
        makers.forEach((_, i) => {
          const x0 = labelW + i * (cell + pad);
          fill(x0, y0, cell, cell, panel);
          const layer = frames[n++].layer;
          for (let y = 0; y < SIZE; y++) {
            for (let x = 0; x < SIZE; x++) {
              const c = layer.get(x, y);
              if (c) fill(x0 + x * scale, y0 + y * scale, scale, scale, c);
            }
          }
        });
      });
    }),
  );
}

// --- Animated tour (GIF) ---------------------------------------------------------------

function demoGif() {
  const scale = 4;
  const w = SIZE * scale;
  const h = SIZE * scale;
  const bg = hex("#F4F1EC");
  const palette = [bg];
  const index = (c) => {
    let i = palette.findIndex((p) => p[0] === c[0] && p[1] === c[1] && p[2] === c[2]);
    if (i < 0) i = palette.push(c) - 1;
    return i;
  };
  const tour = [
    ["spawn", 1],
    ["idle", 2],
    ["idle_look", 1],
    ["sit_down", 1],
    ["work_type", 3],
    ["research_read", 2],
    ["stand_up", 1],
    ["wait_wave", 3],
    ["grabbed", 2],
    ["fly", 3],
    ["land_splat", 1],
    ["dizzy", 2],
    ["walk", 2],
    ["sit_down", 1],
    ["sleep", 2],
    ["stand_up", 1],
    ["leave", 1],
  ];
  const out = [];
  for (const [tag, loops] of tour) {
    const t = frameTags.find((f) => f.name === tag);
    for (let l = 0; l < loops; l++) {
      for (let n = t.from; n <= t.to; n++) {
        const indices = new Uint8Array(w * h);
        const layer = frames[n].layer;
        for (let y = 0; y < h; y++) {
          for (let x = 0; x < w; x++) {
            const c = layer.get(Math.floor(x / scale), Math.floor(y / scale));
            indices[y * w + x] = c ? index(c) : 0;
          }
        }
        out.push({ indices, delayMs: frames[n].duration });
      }
    }
  }
  return gif(w, h, palette, out);
}

// --- Icons -------------------------------------------------------------------------------

/** Clawd idle, cropped to its pixels and scaled (nearest neighbour) into a `size` square. */
function icon(size) {
  const layer = frames[frameTags.find((t) => t.name === "idle").from].layer;
  let [x0, y0, x1, y1] = [SIZE, SIZE, 0, 0];
  for (let y = 0; y < SIZE; y++) {
    for (let x = 0; x < SIZE; x++) {
      if (layer.get(x, y)) {
        x0 = Math.min(x0, x);
        y0 = Math.min(y0, y);
        x1 = Math.max(x1, x + 1);
        y1 = Math.max(y1, y + 1);
      }
    }
  }
  const bw = x1 - x0;
  const bh = y1 - y0;
  const fit = (size * 0.94) / Math.max(bw, bh);
  // Whole-pixel scale where it fits, so the pixel art stays crisp.
  const scale = fit >= 1 ? Math.floor(fit) : fit;
  const dw = Math.round(bw * scale);
  const dh = Math.round(bh * scale);
  const ox = Math.floor((size - dw) / 2);
  const oy = Math.floor((size - dh) / 2);
  return png(
    size,
    size,
    rgbaOf(size, size, (put) => {
      for (let y = 0; y < dh; y++) {
        for (let x = 0; x < dw; x++) {
          const c = layer.get(x0 + Math.floor(x / scale), y0 + Math.floor(y / scale));
          if (c) put(ox + x, oy + y, c);
        }
      }
    }),
  );
}

/** A Windows .ico holding PNG images of the given sizes. */
function ico(sizes) {
  const images = sizes.map((s) => icon(s));
  const header = Buffer.alloc(6 + 16 * sizes.length);
  header.writeUInt16LE(0, 0);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(sizes.length, 4);
  let offset = header.length;
  sizes.forEach((s, i) => {
    const e = 6 + 16 * i;
    header.writeUInt8(s >= 256 ? 0 : s, e);
    header.writeUInt8(s >= 256 ? 0 : s, e + 1);
    header.writeUInt8(0, e + 2);
    header.writeUInt8(0, e + 3);
    header.writeUInt16LE(1, e + 4);
    header.writeUInt16LE(32, e + 6);
    header.writeUInt32LE(images[i].length, e + 8);
    header.writeUInt32LE(offset, e + 12);
    offset += images[i].length;
  });
  return Buffer.concat([header, ...images]);
}

// --- Write -------------------------------------------------------------------------------

const here = (path) => fileURLToPath(new URL(path, import.meta.url));
const write = (path, data) => {
  mkdirSync(here(path.replace(/[^/]+$/, "")), { recursive: true });
  writeFileSync(here(path), data);
};

write("../src/sprites/buddy/atlas.png", atlasPng);
write("../src/sprites/buddy/atlas.json", JSON.stringify(atlasJson, null, 2) + "\n");
write("../../../docs/img/buddy/clips.png", clipsSheet());
write("../../../docs/img/buddy/demo.gif", demoGif());
write("../src-tauri/icons/32x32.png", icon(32));
write("../src-tauri/icons/128x128.png", icon(128));
write("../src-tauri/icons/128x128@2x.png", icon(256));
write("../src-tauri/icons/icon.png", icon(512));
write("../src-tauri/icons/icon.ico", ico([16, 24, 32, 48, 64, 256]));
console.log(`buddy atlas: ${frames.length} frames, ${frameTags.length} tags, ${atlasW}x${atlasH}`);
