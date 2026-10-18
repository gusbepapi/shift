import { describe, expect, it } from "vitest";
import type { LanguageGroupFile } from "@shared/config/config";
import { getGlyphInfo } from "@/workspace/glyphInfo";
import { readCustomLanguageGroups } from "./customLanguageGroups";

function file(name: string, text: string): LanguageGroupFile {
  return {
    name,
    path: `/config/language-groups/${name}`,
    format: name.endsWith(".json") ? "json" : "plist",
    text,
  };
}

describe("readCustomLanguageGroups", () => {
  it("reads JSON files and Glyphs Groups.plist files, resolving glyph names", () => {
    const { groups, problems } = readCustomLanguageGroups(getGlyphInfo(), [
      file(
        "a.json",
        JSON.stringify({
          scripts: [
            { name: "Klingon", groups: [{ name: "Letters", ranges: [["U+F8D0", "U+F8D1"]] }] },
          ],
        }),
      ),
      file("b.plist", "{ languages = ({ name = Accents; list = (aacute, uniF8D0); }); }"),
    ]);

    expect(problems).toEqual([]);
    expect(groups.map(({ id }) => id)).toEqual([
      "custom-klingon-letters",
      "custom-accents-accents",
    ]);
    expect(groups[1]?.baseCodepoints).toEqual([0xe1, 0xf8d0]);
  });

  it("keeps the valid groups of a file and names the file in each problem", () => {
    const { groups, problems } = readCustomLanguageGroups(getGlyphInfo(), [
      file("broken.json", "{"),
      file(
        "some.json",
        JSON.stringify({
          scripts: [{ name: "Test", groups: [{ name: "A", codepoints: [65, "x"] }] }],
        }),
      ),
    ]);

    expect(groups.map(({ id }) => id)).toEqual(["custom-test-a"]);
    expect(problems.map((problem) => problem.split(":")[0])).toEqual(["broken.json", "some.json"]);
  });

  it("lets a later file replace a group with the same identifier", () => {
    const text = (codepoint: number) =>
      JSON.stringify({
        scripts: [{ name: "Test", groups: [{ name: "A", codepoints: [codepoint] }] }],
      });
    const { groups } = readCustomLanguageGroups(getGlyphInfo(), [
      file("a.json", text(65)),
      file("b.json", text(66)),
    ]);

    expect(groups).toHaveLength(1);
    expect(groups[0]?.baseCodepoints).toEqual([66]);
  });
});
