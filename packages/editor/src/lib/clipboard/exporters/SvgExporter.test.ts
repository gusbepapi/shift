import { describe, expect, it } from "vitest";
import type { ClipboardPayload, ShiftContent } from "../types";
import { contentToSvg, payloadJsonFromSvg } from "./SvgExporter";

const on = (x: number, y: number, smooth = false) => ({
  x,
  y,
  pointType: "onCurve" as const,
  smooth,
});
const off = (x: number, y: number) => ({ x, y, pointType: "offCurve" as const, smooth: false });

const payloadFor = (content: ShiftContent): ClipboardPayload => ({
  version: 1,
  format: "shift/glyph-data",
  content,
  metadata: { bounds: {} as never, sourceApp: "shift", timestamp: 0 },
});

const render = (content: ShiftContent) => contentToSvg(content, payloadFor(content));

describe("contentToSvg", () => {
  it("writes a closed line contour with y negated", () => {
    const svg = render({
      contours: [{ closed: true, points: [on(0, 0), on(100, 0), on(100, 200)] }],
    });

    expect(svg).toContain('d="M0 0 L100 0 L100 -200 L0 0 Z"');
    expect(svg).toContain('viewBox="0 -200 100 200"');
  });

  it("writes cubic and quadratic segments from off-curve runs", () => {
    const cubic = render({
      contours: [{ closed: true, points: [on(0, 0), off(0, 50), off(50, 100), on(100, 100)] }],
    });
    const quadratic = render({
      contours: [{ closed: false, points: [on(0, 0), off(50, 100), on(100, 0)] }],
    });

    expect(cubic).toContain("C0 -50 50 -100 100 -100");
    expect(quadratic).toContain('d="M0 0 Q50 -100 100 0"');
    expect(quadratic).not.toContain("Z");
  });

  it("starts a closed contour at its first on-curve point when off-curves lead", () => {
    const svg = render({
      contours: [{ closed: true, points: [off(0, 50), off(50, 100), on(100, 100), on(0, 0)] }],
    });

    expect(svg).toContain('d="M100 -100 L0 0 C0 -50 50 -100 100 -100 Z"');
  });

  it("returns null when no contour has an on-curve point", () => {
    expect(render({ contours: [{ closed: true, points: [off(0, 0), off(1, 1)] }] })).toBeNull();
  });

  it("carries the payload so that it survives escaping", () => {
    const content: ShiftContent = {
      contours: [{ closed: true, points: [on(0, 0), on(10, 0), on(10, 10)] }],
    };
    const payload = payloadFor(content);
    const svg = contentToSvg(content, payload);

    expect(svg).not.toBeNull();
    expect(JSON.parse(payloadJsonFromSvg(svg ?? "") ?? "null")).toEqual(payload);
  });
});
