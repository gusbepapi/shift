import { Validate } from "@shift/validation";
import { Svg } from "../../editor/rendering/Svg";
import type { ClipboardPayload, ContourContent, PointContent, ShiftContent } from "../types";

/** Identifies the element that carries the lossless Shift payload inside the SVG. */
export const SHIFT_METADATA_ID = "shift-glyph-data";

const METADATA_PATTERN = new RegExp(
  `<metadata[^>]*\\bid="${SHIFT_METADATA_ID}"[^>]*>([\\s\\S]*?)</metadata>`,
);

/**
 * Renders portable Shift content as an SVG document for other vector editors.
 *
 * @remarks
 * Font space is y-up and SVG space is y-down, so every y coordinate is negated
 * when it is written. The document also carries the {@link ClipboardPayload} as
 * JSON in a `<metadata>` element, so that pasting back into Shift restores the
 * exact points, point types, and smoothness instead of reparsing the outline.
 *
 * @returns SVG markup, or `null` when no contour can be drawn.
 */
export function contentToSvg(content: ShiftContent, payload: ClipboardPayload): string | null {
  const paths = content.contours.map(contourToPathData).filter((d): d is string => d !== null);
  if (paths.length === 0) return null;

  const points = content.contours.flatMap((contour) => contour.points);
  const xs = points.map(({ x }) => x);
  const ys = points.map(({ y }) => -y);
  const left = Math.min(...xs);
  const top = Math.min(...ys);
  const width = Math.max(Math.max(...xs) - left, 1);
  const height = Math.max(Math.max(...ys) - top, 1);

  const svg = Svg.document(
    [left, top, width, height],
    { width: Svg.number(width), height: Svg.number(height) },
    [
      Svg.element("metadata", { id: SHIFT_METADATA_ID }, [Svg.text(JSON.stringify(payload))]),
      Svg.element("path", { d: paths.join(" "), fill: "#000000", "fill-rule": "nonzero" }),
    ],
  );

  return String(svg);
}

/** Reads the Shift payload back from SVG that {@link contentToSvg} wrote, or `null` if absent. */
export function payloadJsonFromSvg(text: string): string | null {
  const match = METADATA_PATTERN.exec(text);
  const escaped = match?.[1];
  if (escaped === undefined) return null;

  return escaped
    .replaceAll("&lt;", "<")
    .replaceAll("&gt;", ">")
    .replaceAll("&quot;", '"')
    .replaceAll("&apos;", "'")
    .replaceAll("&amp;", "&");
}

/**
 * Converts one contour to SVG path data with y negated, or `null` when it has no on-curve point.
 *
 * Off-curve runs follow the point convention of the source: two off-curves before an
 * on-curve make a cubic, one makes a quadratic, and a longer run is a TrueType spline
 * whose on-curve points are implied at the midpoints.
 */
function contourToPathData(contour: ContourContent): string | null {
  const { points, closed } = contour;
  const first = points.findIndex((point) => Validate.isOnCurve(point));
  if (first < 0) return null;

  // A closed contour may begin with off-curves that belong to its last segment
  const ordered = closed ? [...points.slice(first), ...points.slice(0, first)] : [...points];
  const start = ordered[0];
  if (!start || !Validate.isOnCurve(start)) return null;

  const walk = closed ? [...ordered.slice(1), start] : ordered.slice(1);
  const commands = [`M${format(start)}`];
  let pending: PointContent[] = [];

  for (const point of walk) {
    if (Validate.isOffCurve(point)) {
      pending.push(point);
      continue;
    }

    commands.push(...segment(pending, point));
    pending = [];
  }

  // Trailing off-curves on an open contour have no end point to curve to
  if (closed) commands.push("Z");
  return commands.join(" ");
}

function segment(controls: readonly PointContent[], end: PointContent): string[] {
  const [a, b] = controls;
  if (controls.length === 0) return [`L${format(end)}`];
  if (controls.length === 1 && a) return [`Q${format(a)} ${format(end)}`];
  if (controls.length === 2 && a && b) {
    return [`C${format(a)} ${format(b)} ${format(end)}`];
  }

  // Quadratic spline: consecutive off-curves imply an on-curve point halfway between them
  const commands: string[] = [];
  controls.forEach((control, index) => {
    const next = controls[index + 1];
    const target = next
      ? { x: (control.x + next.x) / 2, y: (control.y + next.y) / 2 }
      : { x: end.x, y: end.y };
    commands.push(`Q${format(control)} ${format(target)}`);
  });
  return commands;
}

function format({ x, y }: { x: number; y: number }): string {
  return `${Svg.number(x)} ${Svg.number(-y)}`;
}
