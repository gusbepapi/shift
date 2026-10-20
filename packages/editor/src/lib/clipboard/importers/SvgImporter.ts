import { Validate } from "@shift/validation";
import { applyMatrix, extractShapes, IDENTITY, viewBoxBottom, type SvgShape } from "./svgShapes";
import type {
  ClipboardImporter,
  ClipboardOffer,
  ContourContent,
  PointContent,
  ShiftContent,
} from "../types";

type PathCommand = {
  type: string;
  args: number[];
};

// A number as SVG writes it: `-1.2-0.5` is two numbers and `.5.5` is two, so spaces are optional
const NUMBER = /[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?/y;

/**
 * Reads the numbers after one path command letter.
 *
 * Arc commands hold two flags that may be written without any separator, as `a1 1 0 011 2`,
 * so the fourth and fifth number of every group of seven are single digits.
 */
function tokeniseArguments(type: string, text: string): number[] {
  const values: number[] = [];
  const isArc = type.toUpperCase() === "A";
  let position = 0;

  while (position < text.length) {
    const separator = /^[\s,]*/.exec(text.slice(position));
    position += separator?.[0].length ?? 0;
    if (position >= text.length) break;

    const slot = values.length % 7;
    if (isArc && (slot === 3 || slot === 4)) {
      const flag = text[position];
      if (flag !== "0" && flag !== "1") break;
      values.push(Number(flag));
      position += 1;
      continue;
    }

    NUMBER.lastIndex = position;
    const match = NUMBER.exec(text);
    if (!match) break;

    values.push(Number(match[0]));
    position = NUMBER.lastIndex;
  }

  return values;
}

export class SvgImporter implements ClipboardImporter {
  readonly id = "svg";

  pick(offers: readonly ClipboardOffer[]): ClipboardOffer | null {
    return (
      offers.find((offer) => offer.text !== undefined && this.#canImportText(offer.text)) ?? null
    );
  }

  import(offer: ClipboardOffer): ShiftContent | null {
    if (offer.text === undefined) return null;
    return this.importText(offer.text);
  }

  canImportText(text: string): boolean {
    return this.#canImportText(text);
  }

  /**
   * Imports SVG markup, a `<path>` element, or raw path data as glyph contours.
   *
   * SVG is y-down and glyph space is y-up, so every y coordinate is mirrored about the
   * bottom edge of the artwork: the root `viewBox` when there is one, otherwise the
   * lowest imported point. The artwork then stands upright on the baseline.
   */
  importText(text: string): ShiftContent | null {
    const trimmed = text.trim();

    const shapes = this.#shapesOf(trimmed);
    const contours = shapes.flatMap((shape) => this.#contoursOf(shape));
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

  #canImportText(text: string): boolean {
    const trimmed = text.trim();
    return (
      trimmed.includes("<svg") || trimmed.includes("<path") || this.#looksLikePathData(trimmed)
    );
  }

  #looksLikePathData(text: string): boolean {
    return /^[MmZzLlHhVvCcSsQqTtAa0-9\s,.\-+eE]+$/.test(text) && /[MmLlCcQqSsAa]/.test(text);
  }

  #shapesOf(text: string): SvgShape[] {
    if (this.#looksLikePathData(text)) return [{ pathData: text, transform: IDENTITY }];

    return extractShapes(text);
  }

  #contoursOf(shape: SvgShape): ContourContent[] {
    const commands = this.#parsePathCommands(shape.pathData);
    if (commands.length === 0) return [];

    return this.#commandsToContours(commands).map((contour) => ({
      ...contour,
      points: contour.points.map((point) => ({
        ...point,
        ...applyMatrix(shape.transform, point.x, point.y),
      })),
    }));
  }

  #parsePathCommands(pathData: string): PathCommand[] {
    const commands: PathCommand[] = [];
    const regex = /([MmZzLlHhVvCcSsQqTtAa])([^MmZzLlHhVvCcSsQqTtAa]*)/g;
    let match: RegExpExecArray | null;

    while ((match = regex.exec(pathData)) !== null) {
      const type = match.at(1);
      if (!type) continue;

      const args = tokeniseArguments(type, match.at(2) ?? "");
      commands.push({ type, args });
    }

    return commands;
  }

  #commandsToContours(commands: PathCommand[]): ContourContent[] {
    const contours: ContourContent[] = [];
    let currentContour: PointContent[] = [];
    let currentX = 0;
    let currentY = 0;
    let startX = 0;
    let startY = 0;
    let lastControlX = 0;
    let lastControlY = 0;

    const pairAt = (values: number[], idx: number): readonly [number, number] | null => {
      const v0 = values[idx];
      const v1 = values[idx + 1];
      if (v0 === undefined || v1 === undefined) return null;
      return [v0, v1];
    };

    const quadAt = (
      values: number[],
      idx: number,
    ): readonly [number, number, number, number] | null => {
      const v0 = values[idx];
      const v1 = values[idx + 1];
      const v2 = values[idx + 2];
      const v3 = values[idx + 3];
      if (v0 === undefined || v1 === undefined || v2 === undefined || v3 === undefined) return null;
      return [v0, v1, v2, v3];
    };

    const sextetAt = (
      values: number[],
      idx: number,
    ): readonly [number, number, number, number, number, number] | null => {
      const v0 = values[idx];
      const v1 = values[idx + 1];
      const v2 = values[idx + 2];
      const v3 = values[idx + 3];
      const v4 = values[idx + 4];
      const v5 = values[idx + 5];
      if (
        v0 === undefined ||
        v1 === undefined ||
        v2 === undefined ||
        v3 === undefined ||
        v4 === undefined ||
        v5 === undefined
      ) {
        return null;
      }
      return [v0, v1, v2, v3, v4, v5];
    };

    const septetAt = (
      values: number[],
      idx: number,
    ): readonly [number, number, number, number, number, number, number] | null => {
      const v0 = values[idx];
      const v1 = values[idx + 1];
      const v2 = values[idx + 2];
      const v3 = values[idx + 3];
      const v4 = values[idx + 4];
      const v5 = values[idx + 5];
      const v6 = values[idx + 6];
      if (
        v0 === undefined ||
        v1 === undefined ||
        v2 === undefined ||
        v3 === undefined ||
        v4 === undefined ||
        v5 === undefined ||
        v6 === undefined
      ) {
        return null;
      }
      return [v0, v1, v2, v3, v4, v5, v6];
    };

    const finishContour = (closed: boolean) => {
      if (currentContour.length > 0) {
        // A path that draws back to its start repeats that point before closing
        const head = currentContour[0];
        const tail = currentContour[currentContour.length - 1];
        if (
          closed &&
          currentContour.length > 2 &&
          head &&
          tail &&
          Validate.isOnCurve(tail) &&
          head.x === tail.x &&
          head.y === tail.y
        ) {
          currentContour.pop();
          currentContour[0] = { ...head, smooth: tail.smooth };
        }
        contours.push({ points: currentContour, closed });
        currentContour = [];
      }
    };

    const addOnCurve = (x: number, y: number, smooth = false) => {
      currentContour.push({ x, y, pointType: "onCurve", smooth });
    };

    const addOffCurve = (x: number, y: number) => {
      currentContour.push({ x, y, pointType: "offCurve", smooth: false });
    };

    for (const cmd of commands) {
      const { type, args } = cmd;
      const isRelative = type === type.toLowerCase();

      switch (type.toUpperCase()) {
        case "M": {
          finishContour(false);
          let idx = 0;
          while (true) {
            const pair = pairAt(args, idx);
            if (!pair) break;

            const [dx, dy] = pair;
            const x = isRelative ? currentX + dx : dx;
            const y = isRelative ? currentY + dy : dy;
            if (idx === 0) {
              startX = x;
              startY = y;
            }
            addOnCurve(x, y);
            currentX = x;
            currentY = y;
            idx += 2;
          }
          break;
        }

        case "L": {
          let idx = 0;
          while (true) {
            const pair = pairAt(args, idx);
            if (!pair) break;

            const [dx, dy] = pair;
            const x = isRelative ? currentX + dx : dx;
            const y = isRelative ? currentY + dy : dy;
            addOnCurve(x, y);
            currentX = x;
            currentY = y;
            idx += 2;
          }
          break;
        }

        case "H": {
          for (const arg of args) {
            const x = isRelative ? currentX + arg : arg;
            addOnCurve(x, currentY);
            currentX = x;
          }
          break;
        }

        case "V": {
          for (const arg of args) {
            const y = isRelative ? currentY + arg : arg;
            addOnCurve(currentX, y);
            currentY = y;
          }
          break;
        }

        case "C": {
          let idx = 0;
          while (true) {
            const segment = sextetAt(args, idx);
            if (!segment) break;

            const [dc1x, dc1y, dc2x, dc2y, dx, dy] = segment;
            const c1x = isRelative ? currentX + dc1x : dc1x;
            const c1y = isRelative ? currentY + dc1y : dc1y;
            const c2x = isRelative ? currentX + dc2x : dc2x;
            const c2y = isRelative ? currentY + dc2y : dc2y;
            const x = isRelative ? currentX + dx : dx;
            const y = isRelative ? currentY + dy : dy;

            addOffCurve(c1x, c1y);
            addOffCurve(c2x, c2y);
            addOnCurve(x, y, true);

            lastControlX = c2x;
            lastControlY = c2y;
            currentX = x;
            currentY = y;
            idx += 6;
          }
          break;
        }

        case "S": {
          let idx = 0;
          while (true) {
            const segment = quadAt(args, idx);
            if (!segment) break;

            const c1x = 2 * currentX - lastControlX;
            const c1y = 2 * currentY - lastControlY;
            const [dc2x, dc2y, dx, dy] = segment;
            const c2x = isRelative ? currentX + dc2x : dc2x;
            const c2y = isRelative ? currentY + dc2y : dc2y;
            const x = isRelative ? currentX + dx : dx;
            const y = isRelative ? currentY + dy : dy;

            addOffCurve(c1x, c1y);
            addOffCurve(c2x, c2y);
            addOnCurve(x, y, true);

            lastControlX = c2x;
            lastControlY = c2y;
            currentX = x;
            currentY = y;
            idx += 4;
          }
          break;
        }

        case "Q": {
          let idx = 0;
          while (true) {
            const segment = quadAt(args, idx);
            if (!segment) break;

            const [dcx, dcy, dx, dy] = segment;
            const cx = isRelative ? currentX + dcx : dcx;
            const cy = isRelative ? currentY + dcy : dcy;
            const x = isRelative ? currentX + dx : dx;
            const y = isRelative ? currentY + dy : dy;

            addOffCurve(cx, cy);
            addOnCurve(x, y, true);

            lastControlX = cx;
            lastControlY = cy;
            currentX = x;
            currentY = y;
            idx += 4;
          }
          break;
        }

        case "T": {
          let idx = 0;
          while (true) {
            const pair = pairAt(args, idx);
            if (!pair) break;

            const cx = 2 * currentX - lastControlX;
            const cy = 2 * currentY - lastControlY;
            const [dx, dy] = pair;
            const x = isRelative ? currentX + dx : dx;
            const y = isRelative ? currentY + dy : dy;

            addOffCurve(cx, cy);
            addOnCurve(x, y, true);

            lastControlX = cx;
            lastControlY = cy;
            currentX = x;
            currentY = y;
            idx += 2;
          }
          break;
        }

        case "Z": {
          finishContour(true);
          currentX = startX;
          currentY = startY;
          break;
        }

        case "A": {
          let idx = 0;
          while (true) {
            const segment = septetAt(args, idx);
            if (!segment) break;

            const [, , , , , dx, dy] = segment;
            const x = isRelative ? currentX + dx : dx;
            const y = isRelative ? currentY + dy : dy;
            addOnCurve(x, y);
            currentX = x;
            currentY = y;
            idx += 7;
          }
          break;
        }
      }

      // A smooth command reflects the previous control point only when a curve preceded it
      if (!"CSQT".includes(type.toUpperCase())) {
        lastControlX = currentX;
        lastControlY = currentY;
      }
    }

    finishContour(false);
    return contours;
  }
}
