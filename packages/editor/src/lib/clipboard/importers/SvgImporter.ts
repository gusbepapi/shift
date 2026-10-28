import type {
  ClipboardImporter,
  ClipboardOffer,
  ContourContent,
  PointContent,
  ShiftContent,
} from "../types";
import { loadEngine } from "../../wasm/engine";
import type { ContourPoints } from "../../wasm/engineModule";
import { applyMatrix, extractShapes, IDENTITY, viewBoxBottom, type SvgShape } from "./svgShapes";

export class SvgImporter implements ClipboardImporter {
  readonly id = "svg";

  pick(offers: readonly ClipboardOffer[]): ClipboardOffer | null {
    return (
      offers.find((offer) => offer.text !== undefined && this.#canImportText(offer.text)) ?? null
    );
  }

  async import(offer: ClipboardOffer): Promise<ShiftContent | null> {
    if (offer.text === undefined) return null;
    return this.importText(offer.text);
  }

  canImportText(text: string): boolean {
    return this.#canImportText(text);
  }

  /**
   * Contours from SVG markup, a `<path>` element, or bare path data.
   *
   * SVG is y-down and glyph space is y-up, so every y coordinate is mirrored about the
   * bottom edge of the artwork: the root `viewBox` when there is one, otherwise the
   * lowest imported point. The artwork then stands upright on the baseline.
   *
   * @remarks
   * shift-font parses the path data, in the window's engine module, which
   * this loads on first use. The basic shapes of Adobe Illustrator and the transform on
   * each element are reduced here, before and after that parse.
   */
  async importText(text: string): Promise<ShiftContent | null> {
    const trimmed = text.trim();

    const shapes = this.#shapesOf(trimmed);
    const contours: ContourContent[] = [];
    for (const shape of shapes) {
      contours.push(...(await this.#contoursOf(shape)));
    }
    if (contours.length === 0) return null;

    const lowest = Math.max(...contours.flatMap((contour) => contour.points.map(({ y }) => y)));
    const bottom = viewBoxBottom(trimmed) ?? lowest;

    return {
      contours: contours.map((contour) => ({
        ...contour,
        points: contour.points.map((point) => ({ ...point, y: bottom - point.y })),
      })),
    };
  }

  #shapesOf(text: string): SvgShape[] {
    if (this.#looksLikePathData(text)) return [{ pathData: text, transform: IDENTITY }];

    return extractShapes(text);
  }

  async #contoursOf(shape: SvgShape): Promise<ContourContent[]> {
    const { svgPathContours } = await loadEngine();
    const codeUnits = Array.from({ length: shape.pathData.length }, (_, index) =>
      shape.pathData.charCodeAt(index),
    );
    const parsed = svgPathContours({ codeUnits });
    if (!parsed || parsed.length === 0) return [];

    return parsed.map(contourContent).map((contour) => ({
      ...contour,
      points: contour.points.map((point) => ({
        ...point,
        ...applyMatrix(shape.transform, point.x, point.y),
      })),
    }));
  }

  #canImportText(text: string): boolean {
    const trimmed = text.trim();
    return (
      trimmed.includes("<svg") || trimmed.includes("<path") || this.#looksLikePathData(trimmed)
    );
  }

  #looksLikePathData(text: string): boolean {
    return /^[MmZzLlHhVvCcSsQqTtAa0-9\s,.\-+eE]+$/.test(text) && /[MmLlCcQqSsAa]/.test(text);
  }
}

/** One parsed subpath as portable content; every point is a corner. */
function contourContent(contour: ContourPoints): ContourContent {
  const points: PointContent[] = contour.pointTypes.map((pointType, index) => ({
    x: contour.coordinates[index * 2]!,
    y: contour.coordinates[index * 2 + 1]!,
    pointType,
    smooth: false,
  }));
  return { points, closed: contour.closed };
}
