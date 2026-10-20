/** One cubic segment: two control points and the end point. */
export type CubicSegment = readonly [number, number, number, number, number, number];

/**
 * Converts an SVG elliptical arc into cubic Bézier segments.
 *
 * @remarks
 * Follows the endpoint-to-centre conversion of the SVG specification, appendix B.2.4, then
 * splits the sweep into pieces of at most a quarter turn, so that each cubic stays within
 * a very small distance of the arc.
 *
 * @param x0 - start point, x.
 * @param y0 - start point, y.
 * @param rx - ellipse radius along its own x axis; the sign is ignored.
 * @param ry - ellipse radius along its own y axis; the sign is ignored.
 * @param rotation - rotation of the ellipse in degrees.
 * @param largeArc - whether the arc spans more than half a turn.
 * @param sweep - whether the arc runs in the direction of increasing angle.
 * @param x - end point, x.
 * @param y - end point, y.
 * @returns the segments, or an empty list when the end point equals the start or a radius is
 * zero, where the specification asks for a straight line.
 */
export function arcToCubics(
  x0: number,
  y0: number,
  rx: number,
  ry: number,
  rotation: number,
  largeArc: boolean,
  sweep: boolean,
  x: number,
  y: number,
): CubicSegment[] {
  let radiusX = Math.abs(rx);
  let radiusY = Math.abs(ry);
  if ((x0 === x && y0 === y) || radiusX === 0 || radiusY === 0) return [];

  const phi = (rotation * Math.PI) / 180;
  const cosPhi = Math.cos(phi);
  const sinPhi = Math.sin(phi);

  // Step 1: the start point in the frame where the ellipse is axis aligned and centred
  const halfDx = (x0 - x) / 2;
  const halfDy = (y0 - y) / 2;
  const x1 = cosPhi * halfDx + sinPhi * halfDy;
  const y1 = -sinPhi * halfDx + cosPhi * halfDy;

  // Radii too small for the end points grow until the arc just fits
  const scale = (x1 * x1) / (radiusX * radiusX) + (y1 * y1) / (radiusY * radiusY);
  if (scale > 1) {
    const factor = Math.sqrt(scale);
    radiusX *= factor;
    radiusY *= factor;
  }

  // Step 2: the centre in that frame
  const rx2 = radiusX * radiusX;
  const ry2 = radiusY * radiusY;
  const numerator = rx2 * ry2 - rx2 * y1 * y1 - ry2 * x1 * x1;
  const denominator = rx2 * y1 * y1 + ry2 * x1 * x1;
  const root = denominator === 0 ? 0 : Math.sqrt(Math.max(0, numerator / denominator));
  const sign = largeArc === sweep ? -1 : 1;
  const centreX1 = (sign * root * radiusX * y1) / radiusY;
  const centreY1 = (-sign * root * radiusY * x1) / radiusX;

  // Step 3: the centre in the original frame
  const centreX = cosPhi * centreX1 - sinPhi * centreY1 + (x0 + x) / 2;
  const centreY = sinPhi * centreX1 + cosPhi * centreY1 + (y0 + y) / 2;

  // Step 4: the start angle and the sweep
  const angle = (ux: number, uy: number, vx: number, vy: number): number => {
    const dot = ux * vx + uy * vy;
    const length = Math.hypot(ux, uy) * Math.hypot(vx, vy);
    const value = Math.acos(Math.min(1, Math.max(-1, dot / length)));
    return ux * vy - uy * vx < 0 ? -value : value;
  };
  const startAngle = angle(1, 0, (x1 - centreX1) / radiusX, (y1 - centreY1) / radiusY);
  let sweepAngle = angle(
    (x1 - centreX1) / radiusX,
    (y1 - centreY1) / radiusY,
    (-x1 - centreX1) / radiusX,
    (-y1 - centreY1) / radiusY,
  );
  if (!sweep && sweepAngle > 0) sweepAngle -= 2 * Math.PI;
  if (sweep && sweepAngle < 0) sweepAngle += 2 * Math.PI;

  const pieces = Math.max(1, Math.ceil(Math.abs(sweepAngle) / (Math.PI / 2) - 1e-9));
  const step = sweepAngle / pieces;
  // Distance of the control points along the tangent for a piece of `step` radians
  const handle = (4 / 3) * Math.tan(step / 4);

  const pointAt = (theta: number): [number, number] => {
    const ex = radiusX * Math.cos(theta);
    const ey = radiusY * Math.sin(theta);
    return [cosPhi * ex - sinPhi * ey + centreX, sinPhi * ex + cosPhi * ey + centreY];
  };
  const tangentAt = (theta: number): [number, number] => {
    const ex = -radiusX * Math.sin(theta);
    const ey = radiusY * Math.cos(theta);
    return [cosPhi * ex - sinPhi * ey, sinPhi * ex + cosPhi * ey];
  };

  const segments: CubicSegment[] = [];
  for (let piece = 0; piece < pieces; piece++) {
    const from = startAngle + piece * step;
    const to = from + step;
    const [startX, startY] = pointAt(from);
    const [startTx, startTy] = tangentAt(from);
    const [endX, endY] = piece === pieces - 1 ? [x, y] : pointAt(to);
    const [endTx, endTy] = tangentAt(to);
    segments.push([
      startX + handle * startTx,
      startY + handle * startTy,
      endX - handle * endTx,
      endY - handle * endTy,
      endX,
      endY,
    ]);
  }
  return segments;
}
