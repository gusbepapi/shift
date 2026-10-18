import { describe, expect, it } from "vitest";
import { groupsFromGlyphsDump, type GlyphsSidebarDump } from "./glyphsSidebar.js";

const info = (name: string, unicode: string | null) => ({
  name,
  unicode,
  unicodes: unicode ? [unicode] : [],
});

const dump: GlyphsSidebarDump = {
  languages: {
    name: "Languages",
    subItems: [
      {
        name: "Latin",
        subItems: [
          { name: "Basic", infos: [info("A", "0041"), info("B", "0042")] },
          {
            name: "Character Sets",
            subItems: [{ name: "Windows 1252", infos: [info("euro", "20AC")] }],
          },
        ],
      },
      { name: "Armenian", infos: [info("Ayb", "0531")] },
      {
        name: "Devanagari",
        subItems: [
          { name: "Consonants", subItems: [{ name: "Basic", infos: [info("ka", "0915")] }] },
          { name: "Half Forms", subItems: [{ name: "Basic", infos: [info("ka-half", null)] }] },
        ],
      },
      { name: "Myanmar", subItems: [{ name: "Letters", infos: [info("ka-my", "1000")] }] },
      { name: "Klingon (pIqaD)", subItems: [{ name: "Letters", infos: [info("a", "F8D0")] }] },
    ],
  },
};

describe("groupsFromGlyphsDump", () => {
  const { groups, problems } = groupsFromGlyphsDump(
    dump,
    (name) => (name === "ka-half" ? 0x94d : null),
    new Set(["Klingon (pIqaD)"]),
  );

  it("keeps declared order, sections, and exact membership", () => {
    expect(groups.map(({ id }) => id)).toEqual([
      "group-latin-basic",
      "group-latin-windows-1252",
      "group-armenian-armenian",
      "group-devanagari-consonants-basic",
      "group-devanagari-half-forms-basic",
      "group-burmese-letters",
    ]);
    expect(groups[1]).toMatchObject({ section: "Character Sets", basis: "exact" });
    expect(groups[0]?.baseCodepoints).toEqual([0x41, 0x42]);
  });

  it("lists a script without subgroups as one group named after it", () => {
    expect(groups.find(({ script }) => script === "Armenian")).toMatchObject({
      name: "Armenian",
      section: null,
    });
  });

  it("qualifies identifiers that would collide and resolves unnumbered glyph names", () => {
    expect(
      groups.find(({ id }) => id === "group-devanagari-half-forms-basic")?.baseCodepoints,
    ).toEqual([0x94d]);
  });

  it("spells scripts as Hyperglot does and skips excluded rows", () => {
    expect(groups.some(({ script }) => script === "Burmese")).toBe(true);
    expect(groups.some(({ script }) => script.startsWith("Klingon"))).toBe(false);
    expect(problems).toEqual([]);
  });
});
