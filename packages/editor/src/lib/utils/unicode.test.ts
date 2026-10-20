import { describe, expect, it } from "vitest";
import { parseCodepointInput } from "./unicode";

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
