/** A drawable SVG element reduced to path data and the transform on the element itself. */
export type SvgShape = {
  pathData: string;
  transform: Matrix;
};

/** An affine transform as the six numbers of the SVG `matrix(a b c d e f)` function. */
export type Matrix = readonly [number, number, number, number, number, number];

export const IDENTITY: Matrix = [1, 0, 0, 1, 0, 0];

// Control-point distance that makes a cubic follow a quarter circle
const KAPPA = 0.5522847498;

// Elements that hold definitions, never drawn artwork
const NON_DRAWN_BLOCKS =
  /<(defs|clipPath|mask|symbol|metadata|style|title|desc)\b[\s\S]*?<\/\1>|<!--[\s\S]*?-->|<\?[\s\S]*?\?>/g;

const SHAPE_TAG = /<(path|polygon|polyline|rect|circle|ellipse|line)\b([^>]*?)\/?>/g;

/**
 * Lists the drawable shapes of SVG markup, in document order.
 *
 * @remarks
 * Adobe Illustrator writes `<polygon>`, `<rect>`, `<circle>` and the other basic
 * shapes besides `<path>`, so each becomes path data here. Only the transform on the
 * element itself applies; transforms of ancestor groups are not tracked.
 */
export function extractShapes(markup: string): SvgShape[] {
  const drawn = markup.replace(NON_DRAWN_BLOCKS, "");
  const shapes: SvgShape[] = [];

  for (const match of drawn.matchAll(SHAPE_TAG)) {
    const [, tag = "", attributes = ""] = match;
    const pathData = pathDataOf(tag, attributes);
    if (!pathData) continue;

    shapes.push({ pathData, transform: parseTransform(attribute(attributes, "transform")) });
  }

  return shapes;
}

/** Returns the bottom edge of the root `viewBox`, or `null` when there is none. */
export function viewBoxBottom(markup: string): number | null {
  const root = /<svg\b[^>]*>/.exec(markup)?.[0];
  const values = attribute(root ?? "", "viewBox")
    ?.trim()
    .split(/[\s,]+/)
    .map(Number);
  if (!values || values.length !== 4 || values.some((value) => !Number.isFinite(value))) {
    return null;
  }

  const [, y = 0, , height = 0] = values;
  return y + height;
}

export function applyMatrix(matrix: Matrix, x: number, y: number): { x: number; y: number } {
  const [a, b, c, d, e, f] = matrix;
  return { x: a * x + c * y + e, y: b * x + d * y + f };
}

/** Reads `matrix`, `translate`, `scale`, `rotate` and `skew` functions, applied left to right. */
export function parseTransform(value: string | null): Matrix {
  if (!value) return IDENTITY;

  let result: Matrix = IDENTITY;
  for (const match of value.matchAll(
    /(matrix|translate|scale|rotate|skewX|skewY)\s*\(([^)]*)\)/g,
  )) {
    const name = match[1];
    const args = (match[2] ?? "")
      .split(/[\s,]+/)
      .filter((part) => part.length > 0)
      .map(Number);
    if (args.some((arg) => !Number.isFinite(arg))) continue;

    result = multiply(result, functionMatrix(name, args));
  }
  return result;
}

function functionMatrix(name: string | undefined, args: number[]): Matrix {
  const [first = 0, second, third, fourth, fifth, sixth] = args;
  switch (name) {
    case "matrix":
      return args.length === 6
        ? [first, second ?? 0, third ?? 0, fourth ?? 0, fifth ?? 0, sixth ?? 0]
        : IDENTITY;
    case "translate":
      return [1, 0, 0, 1, first, second ?? 0];
    case "scale":
      return [first, 0, 0, second ?? first, 0, 0];
    case "rotate": {
      const radians = (first * Math.PI) / 180;
      const cos = Math.cos(radians);
      const sin = Math.sin(radians);
      const rotation: Matrix = [cos, sin, -sin, cos, 0, 0];
      if (second === undefined || third === undefined) return rotation;
      // Rotating about a centre is a translation there, the rotation, and the translation back
      return multiply(multiply([1, 0, 0, 1, second, third], rotation), [
        1,
        0,
        0,
        1,
        -second,
        -third,
      ]);
    }
    case "skewX":
      return [1, 0, Math.tan((first * Math.PI) / 180), 1, 0, 0];
    case "skewY":
      return [1, Math.tan((first * Math.PI) / 180), 0, 1, 0, 0];
    default:
      return IDENTITY;
  }
}

function multiply(left: Matrix, right: Matrix): Matrix {
  const [a1, b1, c1, d1, e1, f1] = left;
  const [a2, b2, c2, d2, e2, f2] = right;
  return [
    a1 * a2 + c1 * b2,
    b1 * a2 + d1 * b2,
    a1 * c2 + c1 * d2,
    b1 * c2 + d1 * d2,
    a1 * e2 + c1 * f2 + e1,
    b1 * e2 + d1 * f2 + f1,
  ];
}

function pathDataOf(tag: string, attributes: string): string | null {
  const number = (name: string): number => Number(attribute(attributes, name) ?? 0);

  switch (tag) {
    case "path":
      return attribute(attributes, "d");
    case "polygon":
    case "polyline": {
      const numbers = (attribute(attributes, "points") ?? "")
        .split(/[\s,]+/)
        .filter((part) => part.length > 0)
        .map(Number);
      if (numbers.length < 4 || numbers.some((value) => !Number.isFinite(value))) return null;

      const pairs: string[] = [];
      for (let index = 0; index + 1 < numbers.length; index += 2) {
        pairs.push(`${numbers[index]} ${numbers[index + 1]}`);
      }
      return `M${pairs.join(" L")}${tag === "polygon" ? " Z" : ""}`;
    }
    case "rect": {
      const [x, y, width, height] = [number("x"), number("y"), number("width"), number("height")];
      if (!(width > 0 && height > 0)) return null;
      return `M${x} ${y} H${x + width} V${y + height} H${x} Z`;
    }
    case "line":
      return `M${number("x1")} ${number("y1")} L${number("x2")} ${number("y2")}`;
    case "circle": {
      const radius = number("r");
      return radius > 0 ? ellipsePath(number("cx"), number("cy"), radius, radius) : null;
    }
    case "ellipse": {
      const [rx, ry] = [number("rx"), number("ry")];
      return rx > 0 && ry > 0 ? ellipsePath(number("cx"), number("cy"), rx, ry) : null;
    }
    default:
      return null;
  }
}

function ellipsePath(cx: number, cy: number, rx: number, ry: number): string {
  const kx = rx * KAPPA;
  const ky = ry * KAPPA;
  return [
    `M${cx + rx} ${cy}`,
    `C${cx + rx} ${cy + ky} ${cx + kx} ${cy + ry} ${cx} ${cy + ry}`,
    `C${cx - kx} ${cy + ry} ${cx - rx} ${cy + ky} ${cx - rx} ${cy}`,
    `C${cx - rx} ${cy - ky} ${cx - kx} ${cy - ry} ${cx} ${cy - ry}`,
    `C${cx + kx} ${cy - ry} ${cx + rx} ${cy - ky} ${cx + rx} ${cy}`,
    "Z",
  ].join(" ");
}

/** Reads one attribute by name; the name must follow whitespace, so `id=` never matches `d=`. */
function attribute(attributes: string, name: string): string | null {
  const match = new RegExp(`(?:^|\\s)${name}\\s*=\\s*(?:"([^"]*)"|'([^']*)')`).exec(attributes);
  return match?.[1] ?? match?.[2] ?? null;
}
