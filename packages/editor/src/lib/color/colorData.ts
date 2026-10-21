import type { ColorData, ColorGlyph, GlyphId, Paint, PaintColor, Rgba } from "@shift/types";

/**
 * Pure edits of the font's colour data.
 *
 * Each function returns the complete next {@link ColorData}, which the caller submits as one
 * undoable `setColor` edit, or `null` when the edit would break a rule Rust enforces: palettes of
 * one length, paints that use existing entries, and layer glyphs that are not colour glyphs.
 * Inputs are never mutated.
 */

/** Parses the JSON the bridge returns and echoes for colour data. */
export function parseColorData(json: string): ColorData {
  return JSON.parse(json) as ColorData;
}

/** Replaces the colour of one entry in one palette. */
export function setPaletteColor(
  data: ColorData,
  paletteIndex: number,
  entryIndex: number,
  color: Rgba,
): ColorData | null {
  const palette = data.palettes[paletteIndex];
  if (!palette || entryIndex < 0 || entryIndex >= palette.colors.length) return null;
  return {
    ...data,
    palettes: data.palettes.map((candidate, index) =>
      index === paletteIndex
        ? { ...candidate, colors: candidate.colors.map((c, i) => (i === entryIndex ? color : c)) }
        : candidate,
    ),
  };
}

/** Appends an entry to every palette, creating the first palette when the font has none. */
export function addPaletteEntry(data: ColorData, color: Rgba): ColorData {
  if (data.palettes.length === 0) {
    return { ...data, palettes: [{ name: null, colors: [color] }] };
  }
  return {
    ...data,
    palettes: data.palettes.map((palette) => ({ ...palette, colors: [...palette.colors, color] })),
  };
}

/** Returns whether any layer of any colour glyph paints with the entry. */
export function paletteEntryInUse(data: ColorData, entryIndex: number): boolean {
  return data.glyphs.some((entry) => paintColors(entry.paint).some((c) => isEntry(c, entryIndex)));
}

/**
 * Removes an entry from every palette and renumbers the later entries in the paints.
 *
 * @returns `null` when a paint still uses the entry, so the caller can say which edit is blocked.
 */
export function removePaletteEntry(data: ColorData, entryIndex: number): ColorData | null {
  const length = data.palettes[0]?.colors.length ?? 0;
  if (entryIndex < 0 || entryIndex >= length || paletteEntryInUse(data, entryIndex)) return null;
  const shift = (color: PaintColor): PaintColor =>
    color.kind === "palette" && color.index > entryIndex
      ? { kind: "palette", index: color.index - 1 }
      : color;
  return {
    palettes: data.palettes.map((palette) => ({
      ...palette,
      colors: palette.colors.filter((_, index) => index !== entryIndex),
    })),
    glyphs: data.glyphs.map((entry) => ({ ...entry, paint: mapPaintColors(entry.paint, shift) })),
  };
}

/** Appends a palette that starts as a copy of the first, or an empty one in an empty font. */
export function addPalette(data: ColorData, name: string | null = null): ColorData {
  const colors = data.palettes[0]?.colors ?? [];
  return { ...data, palettes: [...data.palettes, { name, colors: [...colors] }] };
}

/**
 * Removes a palette.
 *
 * @returns `null` for the last palette while colour glyphs exist, since their paints need entries.
 */
export function removePalette(data: ColorData, paletteIndex: number): ColorData | null {
  if (paletteIndex < 0 || paletteIndex >= data.palettes.length) return null;
  if (data.palettes.length === 1 && data.glyphs.length > 0) return null;
  return { ...data, palettes: data.palettes.filter((_, index) => index !== paletteIndex) };
}

/** Names a palette; `null` removes the name. */
export function renamePalette(
  data: ColorData,
  paletteIndex: number,
  name: string | null,
): ColorData | null {
  if (!data.palettes[paletteIndex]) return null;
  const trimmed = name?.trim() || null;
  return {
    ...data,
    palettes: data.palettes.map((palette, index) =>
      index === paletteIndex ? { ...palette, name: trimmed } : palette,
    ),
  };
}

/**
 * Adds a solid layer on top of a colour glyph, making the glyph a colour glyph when it is not.
 *
 * @returns `null` when the layer glyph is the base glyph or is itself a colour glyph, when the
 * base is already used as a layer, or when the entry does not exist.
 */
export function addColorLayer(
  data: ColorData,
  base: GlyphId,
  layerGlyph: GlyphId,
  entryIndex: number,
): ColorData | null {
  const length = data.palettes[0]?.colors.length ?? 0;
  if (layerGlyph === base || entryIndex < 0 || entryIndex >= length) return null;
  if (data.glyphs.some((entry) => entry.glyph === layerGlyph)) return null;
  if (data.glyphs.some((entry) => layerList(entry.paint).some((l) => l.glyph === base))) {
    return null;
  }
  const layer: LayerPaint = {
    type: "glyph",
    glyph: layerGlyph,
    fill: { type: "solid", color: { kind: "palette", index: entryIndex }, alpha: 1 },
  };
  const existing = data.glyphs.find((entry) => entry.glyph === base);
  const layers = existing ? [...layerList(existing.paint), layer] : [layer];
  return withColorGlyph(data, { glyph: base, paint: { type: "layers", layers } });
}

/** Removes one layer; removing the last layer returns the glyph to a plain outline. */
export function removeColorLayer(
  data: ColorData,
  base: GlyphId,
  layerIndex: number,
): ColorData | null {
  const existing = data.glyphs.find((entry) => entry.glyph === base);
  if (!existing) return null;
  const layers = layerList(existing.paint);
  if (layerIndex < 0 || layerIndex >= layers.length) return null;
  const remaining = layers.filter((_, index) => index !== layerIndex);
  if (remaining.length === 0) return removeColorGlyph(data, base);
  return withColorGlyph(data, { glyph: base, paint: { type: "layers", layers: remaining } });
}

/** Returns a colour glyph to a plain outline. */
export function removeColorGlyph(data: ColorData, base: GlyphId): ColorData | null {
  if (!data.glyphs.some((entry) => entry.glyph === base)) return null;
  return { ...data, glyphs: data.glyphs.filter((entry) => entry.glyph !== base) };
}

/**
 * Changes the palette entry and opacity of a layer whose fill is solid.
 *
 * @returns `null` for a gradient layer, whose stops this edit does not cover, and for an entry
 * that does not exist or an opacity outside 0 to 1.
 */
export function setLayerSolid(
  data: ColorData,
  base: GlyphId,
  layerIndex: number,
  patch: { entryIndex?: number; alpha?: number },
): ColorData | null {
  const existing = data.glyphs.find((entry) => entry.glyph === base);
  if (!existing) return null;
  const layers = layerList(existing.paint);
  const layer = layers[layerIndex];
  if (layer?.fill.type !== "solid") return null;
  const length = data.palettes[0]?.colors.length ?? 0;
  const current = layer.fill.color;
  const entry = patch.entryIndex ?? (current.kind === "palette" ? current.index : 0);
  const alpha = patch.alpha ?? layer.fill.alpha;
  if (entry < 0 || entry >= length || !(alpha >= 0 && alpha <= 1)) return null;
  const next: LayerPaint = {
    ...layer,
    fill: { type: "solid", color: { kind: "palette", index: entry }, alpha },
  };
  return withColorGlyph(data, {
    glyph: base,
    paint: {
      type: "layers",
      layers: layers.map((candidate, index) => (index === layerIndex ? next : candidate)),
    },
  });
}

type LayerPaint = Extract<Paint, { type: "glyph" }>;

function layerList(paint: Paint): LayerPaint[] {
  if (paint.type === "glyph") return [paint];
  if (paint.type === "layers")
    return paint.layers.filter((l): l is LayerPaint => l.type === "glyph");
  return [];
}

function withColorGlyph(data: ColorData, next: ColorGlyph): ColorData {
  const exists = data.glyphs.some((entry) => entry.glyph === next.glyph);
  return {
    ...data,
    glyphs: exists
      ? data.glyphs.map((entry) => (entry.glyph === next.glyph ? next : entry))
      : [...data.glyphs, next],
  };
}

function isEntry(color: PaintColor, entryIndex: number): boolean {
  return color.kind === "palette" && color.index === entryIndex;
}

function paintColors(paint: Paint): PaintColor[] {
  switch (paint.type) {
    case "glyph":
      return paintColors(paint.fill);
    case "layers":
      return paint.layers.flatMap(paintColors);
    case "solid":
      return [paint.color];
    case "linearGradient":
    case "radialGradient":
      return paint.stops.map((stop) => stop.color);
  }
}

function mapPaintColors(paint: Paint, map: (color: PaintColor) => PaintColor): Paint {
  switch (paint.type) {
    case "glyph":
      return { ...paint, fill: mapPaintColors(paint.fill, map) };
    case "layers":
      return { ...paint, layers: paint.layers.map((layer) => mapPaintColors(layer, map)) };
    case "solid":
      return { ...paint, color: map(paint.color) };
    case "linearGradient":
    case "radialGradient":
      return { ...paint, stops: paint.stops.map((stop) => ({ ...stop, color: map(stop.color) })) };
  }
}
