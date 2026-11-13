// Minimal animated GIF89a encoder (global palette, LZW, looping), so the
// sprite generator can export previews without an image library.

/**
 * Encodes frames of palette indices as a looping GIF.
 * @param {number} width
 * @param {number} height
 * @param {number[][]} palette up to 256 [r, g, b] entries
 * @param {{ indices: Uint8Array, delayMs: number }[]} frames
 */
export function gif(width, height, palette, frames) {
  const bits = Math.max(2, Math.ceil(Math.log2(Math.max(2, palette.length))));
  const size = 1 << bits;
  const out = [];
  const u16 = (n) => out.push(n & 0xff, (n >> 8) & 0xff);

  out.push(...Buffer.from("GIF89a"));
  u16(width);
  u16(height);
  out.push(0x80 | ((bits - 1) << 4) | (bits - 1), 0, 0); // global table, background 0
  for (let i = 0; i < size; i++) out.push(...(palette[i] ?? [0, 0, 0]));
  // Loop forever (NETSCAPE2.0).
  out.push(0x21, 0xff, 0x0b, ...Buffer.from("NETSCAPE2.0"), 0x03, 0x01, 0x00, 0x00, 0x00);

  for (const frame of frames) {
    out.push(0x21, 0xf9, 0x04, 0x00);
    u16(Math.round(frame.delayMs / 10));
    out.push(0x00, 0x00);
    out.push(0x2c);
    u16(0);
    u16(0);
    u16(width);
    u16(height);
    out.push(0x00);
    out.push(bits);
    const data = lzw(frame.indices, bits);
    for (let i = 0; i < data.length; i += 255) {
      const block = data.slice(i, i + 255);
      out.push(block.length, ...block);
    }
    out.push(0x00);
  }
  out.push(0x3b);
  return Buffer.from(out);
}

/** GIF-flavoured LZW with variable code size, resetting when the table is full. */
function lzw(indices, minCodeSize) {
  const clear = 1 << minCodeSize;
  const end = clear + 1;
  const bytes = [];
  let acc = 0;
  let accBits = 0;
  let codeSize = minCodeSize + 1;
  const emit = (code) => {
    acc |= code << accBits;
    accBits += codeSize;
    while (accBits >= 8) {
      bytes.push(acc & 0xff);
      acc >>>= 8;
      accBits -= 8;
    }
  };

  let table = new Map();
  let next = end + 1;
  const reset = () => {
    table = new Map();
    next = end + 1;
    codeSize = minCodeSize + 1;
  };

  emit(clear);
  let prefix = indices[0];
  for (let i = 1; i < indices.length; i++) {
    const k = indices[i];
    const key = prefix * 4096 + k;
    const found = table.get(key);
    if (found !== undefined) {
      prefix = found;
      continue;
    }
    emit(prefix);
    if (next < 4096) {
      table.set(key, next++);
      if (next > 1 << codeSize && codeSize < 12) codeSize++;
    } else {
      emit(clear);
      reset();
    }
    prefix = k;
  }
  emit(prefix);
  emit(end);
  if (accBits > 0) bytes.push(acc & 0xff);
  return bytes;
}
