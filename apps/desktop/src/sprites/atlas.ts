/**
 * Aseprite atlas JSON → named clips. Frame tags are the contract between art
 * and code: a clip's name is its tag name, exactly (docs/animation-spec.md).
 * Supports both of Aseprite's export layouts ("array" and "hash" frames).
 */

export interface Frame {
  x: number;
  y: number;
  w: number;
  h: number;
  /** How long the frame shows, in ms. */
  duration: number;
}

export interface Clip {
  name: string;
  frames: Frame[];
  /** Sum of frame durations, in ms. */
  duration: number;
}

export interface Atlas {
  /** Image file name from `meta.image`, relative to the JSON. */
  image: string;
  clips: Map<string, Clip>;
}

interface AsepriteFrame {
  frame: { x: number; y: number; w: number; h: number };
  duration?: number;
}

interface AsepriteJson {
  frames: AsepriteFrame[] | Record<string, AsepriteFrame>;
  meta: {
    image: string;
    frameTags?: { name: string; from: number; to: number; direction?: string }[];
  };
}

/** Used when Aseprite omits a frame duration. */
const DEFAULT_DURATION_MS = 100;

export class AtlasError extends Error {}

export function parseAtlas(json: unknown): Atlas {
  const data = json as AsepriteJson;
  if (!data?.meta?.image || !data.frames) {
    throw new AtlasError("not an Aseprite atlas: missing frames or meta.image");
  }
  // Hash exports keep frames in file order as object keys.
  const raw = Array.isArray(data.frames) ? data.frames : Object.values(data.frames);
  const frames: Frame[] = raw.map(({ frame, duration }) => ({
    x: frame.x,
    y: frame.y,
    w: frame.w,
    h: frame.h,
    duration: duration && duration > 0 ? duration : DEFAULT_DURATION_MS,
  }));

  const clips = new Map<string, Clip>();
  for (const tag of data.meta.frameTags ?? []) {
    if (tag.from < 0 || tag.to >= frames.length || tag.from > tag.to) {
      throw new AtlasError(`frame tag "${tag.name}" is out of range`);
    }
    if (clips.has(tag.name)) {
      throw new AtlasError(`frame tag "${tag.name}" appears twice`);
    }
    let clipFrames = frames.slice(tag.from, tag.to + 1);
    if (tag.direction === "reverse") clipFrames = clipFrames.reverse();
    if (tag.direction === "pingpong" && clipFrames.length > 2) {
      clipFrames = [...clipFrames, ...clipFrames.slice(1, -1).reverse()];
    }
    clips.set(tag.name, {
      name: tag.name,
      frames: clipFrames,
      duration: clipFrames.reduce((sum, f) => sum + f.duration, 0),
    });
  }
  return { image: data.meta.image, clips };
}

/** Throws if any `names` has no clip, so missing art fails loudly at startup. */
export function requireClips(atlas: Atlas, names: readonly string[]): void {
  const missing = names.filter((name) => !atlas.clips.has(name));
  if (missing.length > 0) {
    throw new AtlasError(`atlas is missing animations: ${missing.join(", ")}`);
  }
}

/**
 * The frame of `clip` shown `elapsed` ms after it started, and when (ms after
 * the start) the next frame begins. Looping clips wrap; others hold their
 * last frame and report `null` once finished.
 */
export function frameAt(
  clip: Clip,
  elapsed: number,
  loop: boolean,
): { frame: Frame; nextChangeAt: number | null } {
  const last = clip.frames[clip.frames.length - 1];
  if (clip.duration <= 0 || clip.frames.length === 0) {
    return { frame: last, nextChangeAt: null };
  }
  const t = Math.max(0, elapsed);
  if (!loop && t >= clip.duration) {
    return { frame: last, nextChangeAt: null };
  }
  const cycleStart = loop ? Math.floor(t / clip.duration) * clip.duration : 0;
  let end = cycleStart;
  for (const frame of clip.frames) {
    end += frame.duration;
    if (t < end) {
      // A single-frame loop never changes.
      const changes = loop ? clip.frames.length > 1 : true;
      return { frame, nextChangeAt: changes ? end : null };
    }
  }
  return { frame: last, nextChangeAt: null };
}
