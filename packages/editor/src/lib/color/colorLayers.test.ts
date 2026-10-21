import type { ColorData, GlyphId, Paint } from "@shift/types";
import { describe, expect, it } from "vitest";
import { colorLayers, colorLayersOf, cssColor, resolvePaintColor } from "./colorLayers";

const base = "glyph-base" as GlyphId;
const outline = "glyph-outline" as GlyphId;
const shine = "glyph-shine" as GlyphId;

const paint: Paint = {
  type: "layers",
  layers: [
    {
      type: "glyph",
      glyph: outline,
      fill: { type: "solid", color: { kind: "palette", index: 0 }, alpha: 1 },
    },
    {
      type: "layers",
      layers: [
        {
          type: "glyph",
          glyph: shine,
          fill: {
            type: "linearGradient",
            stops: [
              { offset: 0, color: { kind: "palette", index: 1 }, alpha: 0.8 },
              { offset: 1, color: { kind: "foreground" }, alpha: 1 },
            ],
            p0: { x: 0, y: 0 },
            p1: { x: 0, y: 100 },
            p2: null,
          },
        },
      ],
    },
  ],
};

describe("colorLayers", () => {
  it("lists clipped layers from the bottom up, flattening nested groups", () => {
    const layers = colorLayers(paint);

    expect(layers.map((layer) => layer.glyph)).toEqual([outline, shine]);
    expect(layers.map((layer) => layer.fill.kind)).toEqual(["solid", "linearGradient"]);
  });

  it("returns no layers for a bare fill that has no outline", () => {
    expect(colorLayers({ type: "solid", color: { kind: "foreground" }, alpha: 1 })).toEqual([]);
  });

  it("finds the layers of a base glyph and returns none for a plain glyph", () => {
    const color: ColorData = { palettes: [], glyphs: [{ glyph: base, paint }] };

    expect(colorLayersOf(color, base)).toHaveLength(2);
    expect(colorLayersOf(color, outline)).toEqual([]);
  });
});

describe("resolvePaintColor", () => {
  const palette = [{ r: 200, g: 20, b: 30, a: 255 }];

  it("resolves a palette index and reports the foreground and a missing index as null", () => {
    expect(resolvePaintColor({ kind: "palette", index: 0 }, palette)).toEqual(palette[0]);
    expect(resolvePaintColor({ kind: "palette", index: 4 }, palette)).toBeNull();
    expect(resolvePaintColor({ kind: "foreground" }, palette)).toBeNull();
  });
});

describe("cssColor", () => {
  it("omits the alpha for an opaque colour and multiplies the channel alpha by the paint alpha", () => {
    expect(cssColor({ r: 1, g: 2, b: 3, a: 255 })).toBe("rgb(1 2 3)");
    expect(cssColor({ r: 1, g: 2, b: 3, a: 255 }, 0.5)).toBe("rgb(1 2 3 / 0.5)");
  });
});
