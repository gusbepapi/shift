import { describe, it, expect } from "vitest";
import { SvgImporter } from "./SvgImporter";

describe("SvgImporter", () => {
  const importer = new SvgImporter();
  const requireValue = <T>(value: T | null | undefined): T => {
    expect(value).toBeDefined();
    expect(value).not.toBeNull();
    if (value === null || value === undefined) {
      throw new Error("Expected value to be present");
    }
    return value;
  };
  const requireImport = async (input: string) => requireValue(await importer.importText(input));

  describe("canImportText", () => {
    it("recognizes SVG elements", () => {
      expect(importer.canImportText('<svg width="100" height="100"></svg>')).toBe(true);
    });

    it("recognizes path elements", () => {
      expect(importer.canImportText('<path d="M0,0 L100,100"/>')).toBe(true);
    });

    it("recognizes raw path data", () => {
      expect(importer.canImportText("M0,0 L100,100 Z")).toBe(true);
    });

    it("rejects plain text", () => {
      expect(importer.canImportText("hello world")).toBe(false);
    });

    it("rejects JSON", () => {
      expect(importer.canImportText('{"format": "shift/glyph-data"}')).toBe(false);
    });
  });

  describe("import", () => {
    it("parses simple line path", async () => {
      const result = await requireImport("M0,0 L100,0 L100,100 Z");
      const contour = requireValue(result.contours[0]);

      expect(result.contours).toHaveLength(1);
      expect(contour.closed).toBe(true);
      expect(contour.points.length).toBeGreaterThanOrEqual(3);
    });

    it("parses move and line commands", async () => {
      const result = await requireImport("M10,20 L30,40");
      const contour = requireValue(result.contours[0]);

      expect(requireValue(contour.points[0])).toMatchObject({
        x: 10,
        y: 20,
        pointType: "onCurve",
      });
      expect(requireValue(contour.points[1])).toMatchObject({ x: 30, y: 0 });
    });

    it("parses cubic bezier (C command)", async () => {
      const result = await requireImport("M0,0 C25,50 75,50 100,0");
      const points = requireValue(result.contours[0]).points;

      const offCurvePoints = points.filter((p) => p.pointType === "offCurve");
      expect(offCurvePoints.length).toBeGreaterThan(0);
    });

    it("parses quadratic bezier (Q command)", async () => {
      const result = await requireImport("M0,0 Q50,100 100,0");
      const points = requireValue(result.contours[0]).points;

      const offCurvePoints = points.filter((p) => p.pointType === "offCurve");
      expect(offCurvePoints.length).toBeGreaterThan(0);
    });

    it("parses horizontal line (H command)", async () => {
      const result = await requireImport("M0,0 H100");
      const points = requireValue(result.contours[0]).points;
      const secondPoint = requireValue(points[1]);

      expect(secondPoint.x).toBe(100);
      expect(secondPoint.y).toBe(0);
    });

    it("parses vertical line (V command)", async () => {
      const result = await requireImport("M0,0 V100");
      const points = requireValue(result.contours[0]).points;
      const secondPoint = requireValue(points[1]);

      expect(secondPoint.x).toBe(0);
      expect(secondPoint.y).toBe(0);
      expect(requireValue(points[0]).y).toBe(100);
    });

    it("parses relative commands (lowercase)", async () => {
      const result = await requireImport("M10,10 l20,0 l0,20");
      const points = requireValue(result.contours[0]).points;

      expect(requireValue(points[0])).toMatchObject({ x: 10, y: 20 });
      expect(requireValue(points[1])).toMatchObject({ x: 30, y: 20 });
      expect(requireValue(points[2])).toMatchObject({ x: 30, y: 0 });
    });

    it("parses path from SVG element", async () => {
      const svg = '<path d="M0,0 L100,100"/>';
      const result = await requireImport(svg);

      expect(result.contours).toHaveLength(1);
    });

    it("creates multiple contours for multiple subpaths", async () => {
      const result = await requireImport("M0,0 L100,0 Z M50,50 L150,50 Z");
      const contour0 = requireValue(result.contours[0]);
      const contour1 = requireValue(result.contours[1]);

      expect(result.contours).toHaveLength(2);
      expect(contour0.closed).toBe(true);
      expect(contour1.closed).toBe(true);
    });

    it("returns null for invalid input", async () => {
      const result = await importer.importText("");
      expect(result).toBeNull();
    });

    it("handles smooth curves (S command)", async () => {
      const result = await requireImport("M0,0 C10,20 30,20 40,0 S70,20 80,0");
      const points = requireValue(result.contours[0]).points;
      expect(points.length).toBeGreaterThan(3);
    });

    it("handles smooth quadratic (T command)", async () => {
      const result = await requireImport("M0,0 Q25,50 50,0 T100,0");
      const points = requireValue(result.contours[0]).points;
      expect(points.length).toBeGreaterThan(2);
    });
  });

  describe("orientation", () => {
    it("mirrors y so that artwork stands upright above the viewBox bottom", () => {
      const result = requireImport(
        '<svg viewBox="0 0 100 200"><path d="M0,0 L100,0 L100,50 Z"/></svg>',
      );
      const ys = requireValue(result.contours[0]).points.map((point) => point.y);

      // The top of the artboard is y = 0 in SVG, which is the highest point in glyph space
      expect(ys).toEqual([200, 200, 150]);
    });

    it("mirrors about the lowest point when there is no viewBox", () => {
      const result = requireImport("M0,0 L100,0 L100,50 Z");
      const ys = requireValue(result.contours[0]).points.map((point) => point.y);

      expect(ys).toEqual([50, 50, 0]);
    });
  });

  describe("Adobe Illustrator markup", () => {
    const illustrator = `<?xml version="1.0" encoding="utf-8"?>
<!-- Generator: Adobe Illustrator 29.0.0, SVG Export Plug-In . SVG Version: 9.03 Build 0)  -->
<svg version="1.1" id="Layer_1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" x="0px" y="0px"
	 viewBox="0 0 100 100" style="enable-background:new 0 0 100 100;" xml:space="preserve">
<style type="text/css">
	.st0{fill:#231F20;}
</style>
<polygon class="st0" points="10,10 90,10 90,40"/>
<rect x="20" y="60" class="st0" width="30" height="20"/>
<circle class="st0" cx="75" cy="75" r="10"/>
</svg>`;

    it("reads the shapes although an id attribute precedes any d attribute", () => {
      const result = requireImport(illustrator);

      expect(result.contours).toHaveLength(3);
    });

    it("converts polygons, rectangles and circles to contours", () => {
      const [triangle, rectangle, circle] = requireImport(illustrator).contours;

      expect(requireValue(triangle).closed).toBe(true);
      expect(requireValue(triangle).points).toHaveLength(3);
      expect(requireValue(rectangle).points).toHaveLength(4);
      expect(
        requireValue(circle).points.filter((point) => point.pointType === "offCurve"),
      ).toHaveLength(8);
    });

    it("ignores definitions, styles and comments", () => {
      const result = requireImport(
        '<svg viewBox="0 0 10 10"><defs><path d="M0,0 L9,9"/></defs><path d="M1,1 L2,2 L3,1 Z"/></svg>',
      );

      expect(result.contours).toHaveLength(1);
    });

    it("applies the transform on an element", () => {
      const result = requireImport(
        '<svg viewBox="0 0 100 100"><path transform="translate(10 20)" d="M0,0 L10,0 L10,10 Z"/></svg>',
      );
      const points = requireValue(result.contours[0]).points;

      expect(requireValue(points[0])).toMatchObject({ x: 10, y: 80 });
    });
  });

  describe("closing point", () => {
    it("drops the point that repeats the start of a closed path", () => {
      const result = requireImport("M0,0 L100,0 L100,100 L0,0 Z");

      expect(requireValue(result.contours[0]).points).toHaveLength(3);
    });

    it("keeps off-curve points that curve back to the start", () => {
      const result = requireImport("M0,0 L100,0 C100,50 50,100 0,0 Z");
      const points = requireValue(result.contours[0]).points;

      expect(points).toHaveLength(4);
      expect(points.filter((point) => point.pointType === "offCurve")).toHaveLength(2);
    });
  });

  describe("compact numbers", () => {
    it("splits numbers that are joined by a sign", () => {
      const result = requireImport("M10-20c-1.5-2-3-4.5-4-6.25z");
      const points = requireValue(result.contours[0]).points;

      expect(points).toHaveLength(4);
      expect(requireValue(points[1])).toMatchObject({ x: 8.5, pointType: "offCurve" });
    });

    it("splits numbers that are joined by a second decimal point", () => {
      const result = requireImport("M.5.5L1.5.5L1.5 2z");
      const points = requireValue(result.contours[0]).points;

      expect(points).toHaveLength(3);
      expect(requireValue(points[1])).toMatchObject({ x: 1.5 });
    });

    it("reads arc flags written without separators", () => {
      const result = requireImport("M0,0a5 5 0 011 2l3 0z");
      const points = requireValue(result.contours[0]).points;

      // The arc becomes one cubic of two handles and an end point, then the line adds a point
      expect(points).toHaveLength(5);
      expect(requireValue(points[3])).toMatchObject({ x: 1, pointType: "onCurve" });
    });

    it("reflects the control point of a smooth curve only after a curve", () => {
      const result = requireImport("M0,0 L10,0 S20,10 30,0");
      const points = requireValue(result.contours[0]).points;

      // The first control point of the S segment is the current point, since L precedes it
      expect(requireValue(points[2])).toMatchObject({ x: 10, y: 10, pointType: "offCurve" });
    });
  });
  describe("arcs", () => {
    const extent = (points: { x: number; y: number }[]) => ({
      minX: Math.min(...points.map((point) => point.x)),
      maxX: Math.max(...points.map((point) => point.x)),
      minY: Math.min(...points.map((point) => point.y)),
      maxY: Math.max(...points.map((point) => point.y)),
    });

    it("draws a circle written as two arcs the way Inkscape does", () => {
      const result = requireImport(
        '<svg viewBox="0 0 100 100"><path d="m 90,50 a 40,40 0 0 1 -40,40 40,40 0 0 1 -40,-40 40,40 0 0 1 40,-40 40,40 0 0 1 40,40 z"/></svg>',
      );
      const contour = requireValue(result.contours[0]);
      const onCurve = contour.points.filter((point) => point.pointType === "onCurve");
      const bounds = extent(contour.points);

      expect(onCurve).toHaveLength(4);
      expect(contour.points).toHaveLength(12);
      expect(bounds.minX).toBeCloseTo(10, 3);
      expect(bounds.maxX).toBeCloseTo(90, 3);
      expect(bounds.minY).toBeCloseTo(10, 3);
      expect(bounds.maxY).toBeCloseTo(90, 3);
    });

    it("keeps every curve point at the radius from the centre", () => {
      const result = requireImport(
        '<svg viewBox="0 0 100 100"><path d="M 90,50 A 40,40 0 1 1 10,50 A 40,40 0 1 1 90,50 Z"/></svg>',
      );
      const contour = requireValue(result.contours[0]);

      for (const point of contour.points.filter((candidate) => candidate.pointType === "onCurve")) {
        expect(Math.hypot(point.x - 50, point.y - 50)).toBeCloseTo(40, 3);
      }
    });

    it("splits a half turn into two quarter turns", () => {
      const result = requireImport(
        '<svg viewBox="0 0 100 100"><path d="M 10,50 A 40,40 0 0 1 90,50"/></svg>',
      );
      const contour = requireValue(result.contours[0]);

      expect(contour.closed).toBe(false);
      expect(contour.points).toHaveLength(7);
    });

    it("grows radii that are too small for the end points", () => {
      const result = requireImport(
        '<svg viewBox="0 0 100 100"><path d="M 10,50 A 1,1 0 0 1 90,50"/></svg>',
      );
      const contour = requireValue(result.contours[0]);
      const end = requireValue(contour.points[contour.points.length - 1]);

      expect(end.x).toBeCloseTo(90, 6);
      expect(extent(contour.points).maxY - extent(contour.points).minY).toBeGreaterThan(30);
    });

    it("reads a zero radius as a straight line", () => {
      const result = requireImport(
        '<svg viewBox="0 0 100 100"><path d="M 10,50 A 0,0 0 0 1 90,50"/></svg>',
      );
      const contour = requireValue(result.contours[0]);

      expect(contour.points).toHaveLength(2);
    });
  });
});
