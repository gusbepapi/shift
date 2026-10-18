import { describe, expect, it } from "vitest";
import {
  codepointFromGlyphName,
  parseCodepointPredicate,
  parseCustomGroups,
  parseGlyphsGroupsPlist,
} from "./customGroups.js";

const NO_NAMES = () => null;

describe("parseCustomGroups", () => {
  it("reads codepoints and ranges written as numbers or strings", () => {
    const { groups, problems } = parseCustomGroups(
      JSON.stringify({
        scripts: [
          {
            name: "Klingon",
            groups: [
              {
                name: "Letters",
                codepoints: ["U+F8D0", 0xf8d1, "uniF8D2"],
                ranges: [["0xF8D4", "U+F8D5"]],
              },
              { name: "Digits", section: "Numbers", ranges: [["U+F8F0", "U+F8F2"]] },
            ],
          },
        ],
      }),
    );

    expect(problems).toEqual([]);
    expect(groups.map(({ id }) => id)).toEqual([
      "custom-klingon-letters",
      "custom-klingon-numbers-digits",
    ]);
    expect(groups[0]?.baseCodepoints).toEqual([0xf8d0, 0xf8d1, 0xf8d2, 0xf8d4, 0xf8d5]);
    expect(groups[1]).toMatchObject({ section: "Numbers", basis: "custom" });
    expect(groups[1]?.baseCodepoints).toEqual([0xf8f0, 0xf8f1, 0xf8f2]);
  });

  it("skips an entry with a problem and keeps the rest", () => {
    const { groups, problems } = parseCustomGroups(
      JSON.stringify({
        scripts: [
          {
            name: "Test",
            groups: [
              { name: "Bad", codepoints: ["nonsense"] },
              { name: "Reversed", ranges: [["U+0042", "U+0041"]] },
              { name: "Good", codepoints: ["U+0041"] },
            ],
          },
        ],
      }),
    );

    expect(groups.map(({ name }) => name)).toEqual(["Good"]);
    expect(problems).toEqual(
      expect.arrayContaining([
        'Test Bad: bad codepoint "nonsense"',
        'Test Reversed: bad range [\"U+0042\",\"U+0041\"]',
      ]),
    );
  });

  it("reports text that is not JSON or has no scripts", () => {
    expect(parseCustomGroups("{").problems[0]).toContain("not valid JSON");
    expect(parseCustomGroups("{}").problems).toEqual(['expected an object with a "scripts" array']);
  });

  it("gives groups with the same names distinct identifiers", () => {
    const { groups } = parseCustomGroups(
      JSON.stringify({
        scripts: [
          {
            name: "Test",
            groups: [
              { name: "Letters", codepoints: [65] },
              { name: "Letters", codepoints: [66] },
            ],
          },
        ],
      }),
    );

    expect(groups.map(({ id }) => id)).toEqual(["custom-test-letters", "custom-test-letters-2"]);
  });
});

describe("parseCodepointPredicate", () => {
  it("joins comparisons with AND and OR", () => {
    expect(
      parseCodepointPredicate(
        "(unicodeChar >= 0xF8D0 AND unicodeChar <= 0xF8E9) OR (unicodeChar >= 0xF8F0 AND unicodeChar <= 0xF8F9)",
      ),
    ).toEqual([
      [0xf8d0, 0xf8e9],
      [0xf8f0, 0xf8f9],
    ]);
  });

  it("reads decimal values and the other operators", () => {
    expect(parseCodepointPredicate("unicodeChar == 65")).toEqual([[65, 65]]);
    expect(parseCodepointPredicate("unicodeChar > 65 AND unicodeChar < 70")).toEqual([[66, 69]]);
  });

  it("rejects predicates on other properties", () => {
    expect(() => parseCodepointPredicate('script == "latin"')).toThrow("unsupported predicate");
    expect(() => parseCodepointPredicate("unicodeChar >= 0x41 AND")).toThrow("ends early");
  });
});

describe("codepointFromGlyphName", () => {
  it("reads names that spell their codepoint", () => {
    expect(codepointFromGlyphName("uniF8D0")).toBe(0xf8d0);
    expect(codepointFromGlyphName("u12000")).toBe(0x12000);
    expect(codepointFromGlyphName("U+0041")).toBe(0x41);
    expect(codepointFromGlyphName("aacute")).toBeNull();
    expect(codepointFromGlyphName("ubreve")).toBeNull();
  });
});

describe("parseGlyphsGroupsPlist", () => {
  const plist = `{
    categories = (
      { name = "Ancient writing systems"; subGroup = ({ name = Cuneiform; predicate = "unicodeChar >= 0x12000"; }); }
    );
    // Comment
    languages = (
      {
        name = "Klingon (pIqaD)";
        subGroup = (
          { name = Letters; list = (uniF8D0, uniF8D1, aacute); },
          { name = Digits; list = (uniF8F0); }
        );
      },
      {
        name = Akkadian;
        subGroup = (
          { name = Main; predicate = "unicodeChar >= 0x12000 AND unicodeChar <= 0x12002"; },
          { name = "Nested"; subGroup = ({ name = Deep; list = (u12003); }); }
        );
      },
      { name = Single; list = (uniF8FF, missingGlyph); }
    );
  }`;

  it("reads lists, predicates, nested groups, and single-row entries", () => {
    const { groups, problems } = parseGlyphsGroupsPlist(plist, (name) =>
      name === "aacute" ? 0xe1 : null,
    );

    expect(groups.map(({ id }) => id)).toEqual([
      "custom-klingon-piqad-letters",
      "custom-klingon-piqad-digits",
      "custom-akkadian-main",
      "custom-akkadian-nested-deep",
      "custom-single-single",
    ]);
    expect(groups[0]?.baseCodepoints).toEqual([0xe1, 0xf8d0, 0xf8d1]);
    expect(groups[2]?.baseCodepoints).toEqual([0x12000, 0x12001, 0x12002]);
    expect(groups[3]).toMatchObject({ section: "Nested", name: "Deep" });
    expect(groups[4]).toMatchObject({ script: "Single", name: "Single" });
    expect(problems).toEqual([
      "categories are Glyphs filters, not languages, and were skipped",
      "Single Single: no codepoint for missingGlyph",
    ]);
  });

  it("reports a file that is not a property list", () => {
    expect(parseGlyphsGroupsPlist("{ languages = ( ", NO_NAMES).problems[0]).toContain(
      "not a property list",
    );
    expect(parseGlyphsGroupsPlist("abc", NO_NAMES).problems).toEqual([
      "expected a dictionary at the top level",
    ]);
  });
});
