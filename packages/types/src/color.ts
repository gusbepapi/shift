import type { GlyphId } from "./ids";

/** An eight-bit sRGB colour with straight alpha. */
export interface Rgba {
  r: number;
  g: number;
  b: number;
  a: number;
}

/** One named palette; entry `n` of every palette plays the same role. */
export interface ColorPalette {
  name: string | null;
  colors: Rgba[];
}

/** A paint colour: the text foreground, or an entry of the active palette. */
export type PaintColor = { kind: "foreground" } | { kind: "palette"; index: number };

/** A gradient coordinate in font units. */
export interface ColorPoint {
  x: number;
  y: number;
}

/** One stop of a gradient colour line. */
export interface ColorStop {
  offset: number;
  color: PaintColor;
  alpha: number;
}

/** A fill or a clipped layer of a colour glyph. */
export type Paint =
  | { type: "glyph"; glyph: GlyphId; fill: Paint }
  | { type: "layers"; layers: Paint[] }
  | { type: "solid"; color: PaintColor; alpha: number }
  | {
      type: "linearGradient";
      stops: ColorStop[];
      p0: ColorPoint;
      p1: ColorPoint;
      p2: ColorPoint | null;
    }
  | {
      type: "radialGradient";
      stops: ColorStop[];
      c0: ColorPoint;
      r0: number | null;
      c1: ColorPoint;
      r1: number | null;
    };

/** The paint graph of one colour base glyph. */
export interface ColorGlyph {
  glyph: GlyphId;
  paint: Paint;
}

/** Font-wide colour data: palettes and colour glyph paint graphs. */
export interface ColorData {
  palettes: ColorPalette[];
  glyphs: ColorGlyph[];
}
