import { describe, expect, it } from "vitest";
import { parseCodepoint, parseCodepointInput } from "./unicode";

describe("parseCodepointInput", () => {
  it.each(["0041", "U+0041", "u+0041", "u0041", "0x41", " 41 "])("reads %s as U+0041", (text) => {
    expect(parseCodepointInput(text)).toEqual({ kind: "codepoint", codepoint: 0x41 });
  });

  it("reads blank input as empty", () => {
    expect(parseCodepointInput("  ")).toEqual({ kind: "empty" });
  });

  it.each(["xyz", "U+", "0041 0042", "1234567", "110000", "D800", "DFFF", "-41"])(
    "rejects %s",
    (text) => {
      expect(parseCodepointInput(text)).toEqual({ kind: "invalid" });
    },
  );

  it("accepts the last scalar value", () => {
    expect(parseCodepointInput("10FFFF")).toEqual({ kind: "codepoint", codepoint: 0x10ffff });
  });
});

describe("parseCodepoint", () => {
  it("reads hex with or without a prefix", () => {
    expect(parseCodepoint("0041")).toBe(0x41);
    expect(parseCodepoint("U+00c5")).toBe(0xc5);
    expect(parseCodepoint(" u+1F600 ")).toBe(0x1f600);
    expect(parseCodepoint("0x10FFFF")).toBe(0x10ffff);
  });

  it("rejects text that is not a Unicode scalar value", () => {
    expect(parseCodepoint("")).toBeNull();
    expect(parseCodepoint("U+")).toBeNull();
    expect(parseCodepoint("A?")).toBeNull();
    expect(parseCodepoint("110000")).toBeNull();
    expect(parseCodepoint("D800")).toBeNull();
  });
});
