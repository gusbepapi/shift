import type { ColorData, GlyphId, Paint, PaintColor, Rgba } from "@shift/types";

/** The fill of one colour layer, with palette entries kept as indices. */
export type LayerFill =
  | { kind: "solid"; color: PaintColor; alpha: number }
  | { kind: "linearGradient"; stops: readonly { color: PaintColor; alpha: number }[] }
  | { kind: "radialGradient"; stops: readonly { color: PaintColor; alpha: number }[] };

/** One clipped layer of a colour glyph, in painting order from the bottom. */
export interface ColorLayer {
  /** The glyph whose outline clips the fill. */
  glyph: GlyphId;
  fill: LayerFill;
}

/**
 * Lists the layers of a colour glyph from the bottom up.
 *
 * Nested layer groups are flattened in painting order. A paint that is not a clipped glyph, such
 * as a bare fill with no outline, contributes no layer because it has no outline to show.
 */
export function colorLayers(paint: Paint): ColorLayer[] {
  switch (paint.type) {
    case "layers":
      return paint.layers.flatMap(colorLayers);
    case "glyph": {
      const fill = layerFill(paint.fill);
      return fill ? [{ glyph: paint.glyph, fill }] : [];
    }
    default:
      return [];
  }
}

/** Returns the layers of a base glyph, or an empty list when the glyph has no colour paint. */
export function colorLayersOf(color: ColorData, glyph: GlyphId): ColorLayer[] {
  const entry = color.glyphs.find((candidate) => candidate.glyph === glyph);
  return entry ? colorLayers(entry.paint) : [];
}

/**
 * Resolves a paint colour against one palette.
 *
 * @returns `null` for the text foreground, which has no fixed colour, and for an index the
 * palette does not have.
 */
export function resolvePaintColor(
  color: PaintColor,
  palette: readonly Rgba[] | undefined,
): Rgba | null {
  if (color.kind === "foreground") return null;
  return palette?.[color.index] ?? null;
}

/** Formats a colour as a CSS `rgb()` function, or `rgba()` when it is not opaque. */
export function cssColor(color: Rgba, alpha = 1): string {
  const opacity = (color.a / 255) * alpha;
  return opacity >= 1
    ? `rgb(${color.r} ${color.g} ${color.b})`
    : `rgb(${color.r} ${color.g} ${color.b} / ${Math.round(opacity * 1000) / 1000})`;
}

function layerFill(paint: Paint): LayerFill | null {
  switch (paint.type) {
    case "solid":
      return { kind: "solid", color: paint.color, alpha: paint.alpha };
    case "linearGradient":
      return { kind: "linearGradient", stops: paint.stops };
    case "radialGradient":
      return { kind: "radialGradient", stops: paint.stops };
    default:
      return null;
  }
}
