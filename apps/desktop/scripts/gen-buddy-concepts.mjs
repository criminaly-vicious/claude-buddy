// Draws static concepts for the original buddy character (#19, ADR-0005) and
// writes one preview sheet per concept to docs/img/concepts/<name>.png: the
// poses idle, work_type, wait_wave and sleep at 32x32, enlarged 8x, on a light
// and a dark background, labelled.
//
// Everything is drawn by code from simple shapes: a body mask is filled, then
// outlined and shaded automatically, then faces, limbs and props are added.
// The chosen concept becomes the full animation set in a later step.
//
// Usage: node scripts/gen-buddy-concepts.mjs
import { mkdirSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { png } from "./png.mjs";

const SIZE = 32;

// --- Palette ------------------------------------------------------------------

const hex = (h) => [parseInt(h.slice(1, 3), 16), parseInt(h.slice(3, 5), 16), parseInt(h.slice(5, 7), 16)];
const C = {
  body: hex("#D97757"),
  light: hex("#EE9A7C"),
  shade: hex("#B65C40"),
  outline: hex("#6E3022"),
  eye: hex("#2A1712"),
  shine: hex("#FFFFFF"),
  cheek: hex("#F0A08E"),
  leaf: hex("#7FB069"),
  leafDark: hex("#4E7A3E"),
  metal: hex("#4A505C"),
  metalLight: hex("#7C8494"),
  metalDark: hex("#2C3038"),
  glow: hex("#BFE8FF"),
  paper: hex("#FFFFFF"),
  ink: hex("#2A1712"),
  alert: hex("#E5484D"),
  zzz: hex("#8FA8FF"),
};

// --- A 32x32 sprite: palette colours, null = transparent ---------------------

class Sprite {
  constructor() {
    this.px = Array.from({ length: SIZE * SIZE }, () => null);
    /** Pixels that belong to the character's body (outlined and shaded together). */
    this.mask = new Uint8Array(SIZE * SIZE);
  }
  inside(x, y) {
    return x >= 0 && y >= 0 && x < SIZE && y < SIZE;
  }
  set(x, y, c) {
    if (this.inside(x, y)) this.px[y * SIZE + x] = c;
  }
  get(x, y) {
    return this.inside(x, y) ? this.px[y * SIZE + x] : null;
  }
  /** Adds a pixel to the body mask. */
  body(x, y) {
    if (this.inside(x, y)) this.mask[y * SIZE + x] = 1;
  }
  isBody(x, y) {
    return this.inside(x, y) && this.mask[y * SIZE + x] === 1;
  }
  ellipse(cx, cy, rx, ry) {
    for (let y = Math.floor(cy - ry); y <= Math.ceil(cy + ry); y++) {
      for (let x = Math.floor(cx - rx); x <= Math.ceil(cx + rx); x++) {
        const dx = (x + 0.5 - cx) / rx;
        const dy = (y + 0.5 - cy) / ry;
        if (dx * dx + dy * dy <= 1) this.body(x, y);
      }
    }
  }
  rect(x0, y0, w, h) {
    for (let y = y0; y < y0 + h; y++) for (let x = x0; x < x0 + w; x++) this.body(x, y);
  }
  /** Fills the body mask with the body colour, a 1 px outline around it, and light/shade rims. */
  paintBody() {
    for (let y = 0; y < SIZE; y++) {
      for (let x = 0; x < SIZE; x++) {
        if (!this.isBody(x, y)) continue;
        const lit = !this.isBody(x - 1, y - 1) && this.isBody(x + 1, y + 1);
        const dark = !this.isBody(x + 1, y + 1) || !this.isBody(x, y + 2);
        this.set(x, y, dark ? C.shade : lit ? C.light : C.body);
      }
    }
    for (let y = 0; y < SIZE; y++) {
      for (let x = 0; x < SIZE; x++) {
        if (this.isBody(x, y)) continue;
        const touches = [
          [1, 0],
          [-1, 0],
          [0, 1],
          [0, -1],
        ].some(([dx, dy]) => this.isBody(x + dx, y + dy));
        if (touches) this.set(x, y, C.outline);
      }
    }
  }
  /** Draws rows of characters; each character maps to a colour, "." is skipped. */
  stamp(x0, y0, rows, colours) {
    rows.forEach((row, dy) => {
      [...row].forEach((ch, dx) => {
        if (ch !== "." && colours[ch]) this.set(x0 + dx, y0 + dy, colours[ch]);
      });
    });
  }
}

// --- Props --------------------------------------------------------------------

/** Laptop seen from the front (the lid's back), its keyboard edge at y. */
function laptop(s, y) {
  s.stamp(8, y - 9, [
    ".kkkkkkkkkkkkkk.",
    "kmmmmmmmmmmmmmmk",
    "kmMMMMMMMMMMMMmk",
    "kmMMMMMMMMMMMMmk",
    "kmMMMMMwwMMMMMmk",
    "kmMMMMMwwMMMMMmk",
    "kmMMMMMMMMMMMMmk",
    "kmmmmmmmmmmmmmmk",
    "kkkkkkkkkkkkkkkk",
    "kmmmmmmmmmmmmmmk",
    ".kkkkkkkkkkkkkk.",
  ], { k: C.metalDark, m: C.metalLight, M: C.metal, w: C.glow });
}

/** Speech bubble with a red "!", top-left at (x, y), tail at the bottom right. */
function alertBubble(s, x, y) {
  s.stamp(x, y, [
    ".kkkkk.",
    "kpppppk",
    "kpprppk",
    "kpprppk",
    "kpprppk",
    "kpppppk",
    "kpprppk",
    "kpppppk",
    ".kkkkpk",
    "....kpk",
    ".....kk",
  ], { k: C.ink, p: C.paper, r: C.alert });
}

function zees(s, x, y) {
  s.stamp(x, y, ["zzzzz", "...z.", "..z..", ".z...", "zzzzz"], { z: C.zzz });
  s.stamp(x + 6, y - 6, ["zzzz", "..z.", ".z..", "zzzz"], { z: C.zzz });
}

// --- Faces ----------------------------------------------------------------------

function eyesOpen(s, lx, rx, y) {
  for (const x of [lx, rx]) {
    s.set(x, y, C.eye);
    s.set(x + 1, y, C.eye);
    s.set(x, y + 1, C.eye);
    s.set(x + 1, y + 1, C.eye);
    s.set(x, y, C.shine);
  }
}

function eyesDown(s, lx, rx, y) {
  // Looking at the screen: eyes lower and half shut.
  for (const x of [lx, rx]) {
    s.set(x, y + 1, C.eye);
    s.set(x + 1, y + 1, C.eye);
  }
}

function eyesClosed(s, lx, rx, y) {
  for (const x of [lx, rx]) {
    s.set(x, y + 1, C.eye);
    s.set(x + 1, y + 1, C.eye);
    s.set(x - 1, y, C.eye);
    s.set(x + 2, y, C.eye);
  }
}

function cheeks(s, lx, rx, y) {
  for (const x of [lx - 1, rx + 1]) {
    s.set(x, y, C.cheek);
    s.set(x + 1, y, C.cheek);
  }
}

// --- Concepts -------------------------------------------------------------------
//
// Each concept knows how to draw its body for three stances (stand, sit, lie)
// and where its face goes. The four poses are built from those.

const CONCEPTS = [
  {
    name: "bean",
    title: "bean",
    blurb: "a round bean on short legs",
    stand(s) {
      s.ellipse(16, 17.5, 8, 9.5);
      s.rect(11, 26, 3, 3); // legs
      s.rect(18, 26, 3, 3);
      return { eyes: [12, 18, 14], cheeks: null, arms: [[7, 18], [24, 18]] };
    },
    sit(s) {
      s.ellipse(16, 20.5, 8.5, 8.5);
      return { eyes: [12, 18, 16], cheeks: null };
    },
    lie(s) {
      s.ellipse(16, 24.5, 11, 5.5);
      return { eyes: [11, 19, 22], cheeks: null };
    },
  },
  {
    name: "sprout",
    title: "sprout",
    blurb: "a round buddy with a leaf antenna",
    antenna: true,
    stand(s) {
      s.ellipse(16, 19.5, 9, 8);
      s.rect(10, 26, 4, 2); // flat feet
      s.rect(18, 26, 4, 2);
      return { eyes: [12, 18, 17], cheeks: null, arms: [[6, 20], [25, 20]], top: 11 };
    },
    sit(s) {
      s.ellipse(16, 21.5, 9.5, 7.5);
      return { eyes: [12, 18, 19], cheeks: null, top: 14 };
    },
    lie(s) {
      s.ellipse(16, 24.5, 11, 5.5);
      return { eyes: [11, 19, 22], cheeks: null, top: 19, lying: true };
    },
  },
  {
    name: "puff",
    title: "puff",
    blurb: "a fluffy puff with rosy cheeks",
    fluffy: true,
    stand(s) {
      s.ellipse(16, 17.5, 9, 9);
      fluff(s, 16, 17.5, 9, 9);
      s.rect(12, 27, 2, 2);
      s.rect(18, 27, 2, 2);
      return { eyes: [12, 18, 15], cheeks: 18, arms: [[5, 19], [26, 19]] };
    },
    sit(s) {
      s.ellipse(16, 20.5, 9.5, 8.5);
      fluff(s, 16, 20.5, 9.5, 8.5);
      return { eyes: [12, 18, 17], cheeks: 20 };
    },
    lie(s) {
      s.ellipse(16, 24.5, 11, 5.5);
      fluff(s, 16, 24.5, 11, 5.5);
      return { eyes: [11, 19, 22], cheeks: 24 };
    },
  },
];

/** Bumps around an ellipse, for a fluffy outline. */
function fluff(s, cx, cy, rx, ry) {
  for (let a = 0; a < 360; a += 30) {
    const t = (a * Math.PI) / 180;
    if (Math.sin(t) > 0.7) continue; // keep the bottom smooth where it sits
    s.ellipse(cx + Math.cos(t) * rx, cy + Math.sin(t) * ry, 1.6, 1.6);
  }
}

/** The sprout's leaf antenna, rooted at the body's top (x 16). */
function antenna(s, top, lying) {
  if (lying) {
    s.stamp(19, top - 3, ["..ll", ".lLl", "s..."], { l: C.leaf, L: C.leafDark, s: C.outline });
    return;
  }
  s.stamp(14, top - 7, [
    "..lll.",
    ".lLLll",
    ".llL..",
    "..s...",
    "..s...",
    "..s...",
    "..s...",
  ], { l: C.leaf, L: C.leafDark, s: C.outline });
}

function arms(s, list) {
  for (const [x, y] of list) s.rect(x, y, 2, 3);
}

const POSES = [
  {
    name: "idle",
    draw(concept) {
      const s = new Sprite();
      const f = concept.stand(s);
      arms(s, f.arms);
      s.paintBody();
      if (concept.antenna) antenna(s, f.top);
      eyesOpen(s, ...f.eyes);
      if (f.cheeks) cheeks(s, f.eyes[0], f.eyes[1], f.cheeks);
      return s;
    },
  },
  {
    name: "work_type",
    draw(concept) {
      const s = new Sprite();
      const f = concept.sit(s);
      s.rect(9, 26, 3, 2); // hands on the keyboard, either side of the lid
      s.rect(20, 26, 3, 2);
      s.paintBody();
      if (concept.antenna) antenna(s, f.top);
      eyesDown(s, ...f.eyes);
      if (f.cheeks) cheeks(s, f.eyes[0], f.eyes[1], f.cheeks);
      // Screen glow on the face.
      s.set(f.eyes[0] + 2, f.eyes[2] + 3, C.glow);
      s.set(f.eyes[1] - 1, f.eyes[2] + 3, C.glow);
      laptop(s, 30);
      return s;
    },
  },
  {
    name: "wait_wave",
    draw(concept) {
      const s = new Sprite();
      const f = concept.stand(s);
      const [left, right] = f.arms;
      s.rect(left[0], left[1], 2, 3);
      // Right arm raised: a diagonal from the shoulder up.
      s.rect(right[0], right[1] - 3, 2, 3);
      s.rect(right[0] + 1, right[1] - 6, 2, 3);
      s.paintBody();
      if (concept.antenna) antenna(s, f.top);
      eyesOpen(s, ...f.eyes);
      if (f.cheeks) cheeks(s, f.eyes[0], f.eyes[1], f.cheeks);
      // Small open mouth.
      s.set(15, f.eyes[2] + 4, C.eye);
      s.set(16, f.eyes[2] + 4, C.eye);
      alertBubble(s, 0, 0);
      return s;
    },
  },
  {
    name: "sleep",
    draw(concept) {
      const s = new Sprite();
      const f = concept.lie(s);
      s.paintBody();
      if (concept.antenna) antenna(s, f.top, true);
      eyesClosed(s, ...f.eyes);
      if (f.cheeks) cheeks(s, f.eyes[0], f.eyes[1], f.cheeks);
      zees(s, 22, 13);
      return s;
    },
  },
];

// --- Tiny 3x5 font for labels ---------------------------------------------------

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
  _: ["...", "...", "...", "...", "###"],
  " ": ["...", "...", "...", "...", "..."],
};

// --- Preview sheet ----------------------------------------------------------------

const SCALE = 8;
const PAD = 16;
const LABEL = 3; // font pixel size
const CELL = SIZE * SCALE;
const BACKGROUNDS = [hex("#F4F1EC"), hex("#2B2D33")];

function sheet(concept) {
  const sprites = POSES.map((pose) => pose.draw(concept));
  const titleH = 5 * LABEL + PAD;
  const labelH = 5 * LABEL + PAD;
  const w = PAD + POSES.length * (CELL + PAD);
  const h = PAD + titleH + BACKGROUNDS.length * CELL + labelH + PAD;
  const rgba = new Uint8Array(w * h * 4);
  const fill = (x0, y0, ww, hh, [r, g, b]) => {
    for (let y = y0; y < y0 + hh; y++) {
      for (let x = x0; x < x0 + ww; x++) rgba.set([r, g, b, 255], (y * w + x) * 4);
    }
  };
  const text = (x0, y0, str, colour) => {
    [...str].forEach((ch, i) => {
      const glyph = FONT[ch] ?? FONT[" "];
      glyph.forEach((row, gy) =>
        [...row].forEach((on, gx) => {
          if (on === "#") fill(x0 + (i * 4 + gx) * LABEL, y0 + gy * LABEL, LABEL, LABEL, colour);
        }),
      );
    });
  };
  fill(0, 0, w, h, hex("#FFFFFF"));
  text(PAD, PAD, `${concept.title}  ${concept.blurb}`, hex("#2A1712"));
  sprites.forEach((sprite, i) => {
    const x0 = PAD + i * (CELL + PAD);
    BACKGROUNDS.forEach((bg, row) => {
      const y0 = PAD + titleH + row * CELL;
      fill(x0, y0, CELL, CELL, bg);
      for (let y = 0; y < SIZE; y++) {
        for (let x = 0; x < SIZE; x++) {
          const c = sprite.get(x, y);
          if (c) fill(x0 + x * SCALE, y0 + y * SCALE, SCALE, SCALE, c);
        }
      }
    });
    text(x0, PAD + titleH + BACKGROUNDS.length * CELL + PAD / 2, POSES[i].name, hex("#2A1712"));
  });
  return png(w, h, rgba);
}

const outDir = fileURLToPath(new URL("../../../docs/img/concepts/", import.meta.url));
mkdirSync(outDir, { recursive: true });
for (const concept of CONCEPTS) {
  writeFileSync(`${outDir}${concept.name}.png`, sheet(concept));
  console.log(`concept sheet: docs/img/concepts/${concept.name}.png`);
}
