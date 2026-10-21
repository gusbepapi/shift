import type { ColorData, GlyphId, Rgba } from "@shift/types";
import { describe, expect, it } from "vitest";
import {
  addColorLayer,
  addPalette,
  addPaletteEntry,
  paletteEntryInUse,
  parseColorData,
  removeColorGlyph,
  removeColorLayer,
  removePalette,
  removePaletteEntry,
  renamePalette,
  setLayerSolid,
  setPaletteColor,
} from "./colorData";

const base = "base" as GlyphId;
const other = "other" as GlyphId;
const fill = "fill" as GlyphId;
const red: Rgba = { r: 255, g: 0, b: 0, a: 255 };
const blue: Rgba = { r: 0, g: 0, b: 255, a: 255 };

const empty: ColorData = { palettes: [], glyphs: [] };

function layered(): ColorData {
  const withEntries = addPaletteEntry(addPaletteEntry(empty, red), blue);
  return addColorLayer(withEntries, base, fill, 1)!;
}

describe("palette edits", () => {
  it("creates the first palette from the first entry and extends every palette afterwards", () => {
    const one = addPaletteEntry(empty, red);
    const two = addPaletteEntry(addPalette(one, "Dark"), blue);

    expect(one.palettes).toEqual([{ name: null, colors: [red] }]);
    expect(two.palettes.map((p) => p.colors.length)).toEqual([2, 2]);
  });

  it("recolours one entry of one palette without touching the others", () => {
    const two = addPalette(addPaletteEntry(empty, red), "Dark");

    const next = setPaletteColor(two, 1, 0, blue)!;

    expect(next.palettes[0].colors[0]).toEqual(red);
    expect(next.palettes[1].colors[0]).toEqual(blue);
    expect(setPaletteColor(two, 1, 5, blue)).toBeNull();
  });

  it("names a palette, trimming blanks to no name", () => {
    const one = addPaletteEntry(empty, red);

    expect(renamePalette(one, 0, "  Light ")!.palettes[0].name).toBe("Light");
    expect(renamePalette(one, 0, "   ")!.palettes[0].name).toBeNull();
    expect(renamePalette(one, 3, "x")).toBeNull();
  });

  it("keeps the last palette while a colour glyph needs it", () => {
    const data = layered();

    expect(removePalette(data, 0)).toBeNull();
    expect(removePalette(addPalette(data), 0)!.palettes).toHaveLength(1);
  });
});

describe("entry removal", () => {
  it("refuses an entry a layer paints with and renumbers the later ones when it is free", () => {
    const data = layered();

    expect(paletteEntryInUse(data, 1)).toBe(true);
    expect(removePaletteEntry(data, 1)).toBeNull();

    const next = removePaletteEntry(data, 0)!;
    expect(next.palettes[0].colors).toEqual([blue]);
    expect(next.glyphs[0].paint).toMatchObject({
      layers: [{ fill: { color: { kind: "palette", index: 0 } } }],
    });
  });
});

describe("colour layers", () => {
  it("makes a plain glyph a colour glyph and appends later layers on top", () => {
    const first = layered();
    const second = addColorLayer(first, base, other, 0)!;

    expect(first.glyphs).toHaveLength(1);
    expect(second.glyphs[0].paint).toMatchObject({
      type: "layers",
      layers: [{ glyph: fill }, { glyph: other }],
    });
  });

  it("rejects layers Rust would reject", () => {
    const data = layered();

    expect(addColorLayer(data, base, base, 0)).toBeNull();
    expect(addColorLayer(data, base, other, 9)).toBeNull();
    expect(addColorLayer(data, other, base, 0)).toBeNull();
    expect(
      addColorLayer(addColorLayer(data, other, "third" as GlyphId, 0)!, "x" as GlyphId, other, 0),
    ).toBeNull();
  });

  it("removes a layer and returns the glyph to an outline after the last one", () => {
    const two = addColorLayer(layered(), base, other, 0)!;

    expect(removeColorLayer(two, base, 0)!.glyphs[0].paint).toMatchObject({
      layers: [{ glyph: other }],
    });
    expect(removeColorLayer(layered(), base, 0)!.glyphs).toEqual([]);
    expect(removeColorLayer(layered(), base, 4)).toBeNull();
    expect(removeColorGlyph(layered(), base)!.glyphs).toEqual([]);
  });

  it("changes a solid layer's entry and opacity and refuses out-of-range values", () => {
    const data = layered();

    const next = setLayerSolid(data, base, 0, { entryIndex: 0, alpha: 0.5 })!;
    expect(next.glyphs[0].paint).toMatchObject({
      layers: [{ fill: { type: "solid", color: { index: 0 }, alpha: 0.5 } }],
    });
    expect(setLayerSolid(data, base, 0, { alpha: 1.5 })).toBeNull();
    expect(setLayerSolid(data, base, 0, { entryIndex: 7 })).toBeNull();
  });

  it("leaves the input untouched", () => {
    const data = layered();
    const snapshot = JSON.stringify(data);

    setLayerSolid(data, base, 0, { alpha: 0.2 });
    removeColorLayer(data, base, 0);
    addPaletteEntry(data, red);

    expect(JSON.stringify(data)).toBe(snapshot);
  });
});

describe("parseColorData", () => {
  it("reads the bridge JSON", () => {
    expect(parseColorData('{"palettes":[],"glyphs":[]}')).toEqual(empty);
  });
});
