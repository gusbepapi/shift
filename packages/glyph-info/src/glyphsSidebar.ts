import { codepointFromGlyphName, type GlyphNameResolver } from "./customGroups.js";
import type { LanguageGroup } from "./types.js";

/** One row of the dump that `DumpLanguagesSidebar.py` writes from Glyphs' Languages sidebar. */
export interface GlyphsSidebarItem {
  name: string | null;
  infos?: readonly { name: string | null; unicode: string | null; unicodes: readonly string[] }[];
  coverage?: readonly string[];
  subItems?: readonly GlyphsSidebarItem[];
}

export interface GlyphsSidebarDump {
  languages: GlyphsSidebarItem;
}

export interface GlyphsSidebarGroups {
  groups: LanguageGroup[];
  problems: string[];
}

/**
 * Spells a script the way Hyperglot does, so a Glyphs script joins the languages of the same script.
 *
 * @remarks
 * Glyphs names its rows «Myanmar» and «Ethiopic», where Hyperglot says «Burmese» and «Geʽez».
 */
const SCRIPT_ALIASES: Readonly<Record<string, string>> = {
  Myanmar: "Burmese",
  Ethiopic: "Geʽez",
  "N'ko": "N'Ko",
  "N’ko": "N'Ko",
};

/**
 * Converts a dump of Glyphs' Languages sidebar into groups.
 *
 * @remarks
 * Glyphs lists a script as a row that holds groups, or as a single row with no groups. A row inside
 * a group is a section, such as «Character Sets». A group's glyphs are read from their Unicode values;
 * glyphs without one cannot be tracked by codepoint and are left out.
 *
 * @param dump - The parsed JSON file.
 * @param resolveGlyphName - Resolves a listed glyph name when the dump carries no Unicode value for it.
 * @param exclude - Script names to skip, such as entries the user added to Glyphs themselves.
 */
export function groupsFromGlyphsDump(
  dump: GlyphsSidebarDump,
  resolveGlyphName: GlyphNameResolver,
  exclude: ReadonlySet<string> = new Set(),
): GlyphsSidebarGroups {
  const entries: { script: string; section: string | null; name: string; codepoints: number[] }[] =
    [];
  const problems: string[] = [];

  const collect = (
    item: GlyphsSidebarItem,
    script: string,
    section: string | null,
    isScriptRow: boolean,
  ): void => {
    const name = item.name ?? "";
    const children = item.subItems ?? [];

    if (children.length > 0) {
      for (const child of children) {
        const childHasChildren = (child.subItems ?? []).length > 0;
        collect(child, script, childHasChildren ? (child.name ?? null) : section, false);
      }
      return;
    }

    const codepoints = codepointsOf(item, resolveGlyphName);
    if (codepoints.length === 0) {
      problems.push(`${script} ${name}: no glyph with a Unicode value`);
      return;
    }

    entries.push({
      script,
      section: isScriptRow ? null : section,
      name: isScriptRow ? script : name,
      codepoints,
    });
  };

  for (const row of dump.languages.subItems ?? []) {
    const rawName = row.name ?? "";
    if (rawName === "" || exclude.has(rawName)) continue;

    collect(row, SCRIPT_ALIASES[rawName] ?? rawName, null, (row.subItems ?? []).length === 0);
  }

  const occurrences = new Map<string, number>();
  for (const entry of entries) {
    const key = `${entry.script}\u0000${entry.name}`;
    occurrences.set(key, (occurrences.get(key) ?? 0) + 1);
  }

  const groups: LanguageGroup[] = [];
  const taken = new Set<string>();
  for (const entry of entries) {
    const repeated = (occurrences.get(`${entry.script}\u0000${entry.name}`) ?? 0) > 1;
    const parts =
      repeated && entry.section
        ? [entry.script, entry.section, entry.name]
        : [entry.script, entry.name];
    const baseId = `group-${parts.map(slugify).join("-")}`;

    let id = baseId;
    for (let suffix = 2; taken.has(id); suffix += 1) id = `${baseId}-${suffix}`;
    taken.add(id);

    groups.push({
      id,
      name: entry.name,
      script: entry.script,
      section: entry.section,
      baseCodepoints: [...new Set(entry.codepoints)].sort((left, right) => left - right),
      basis: "exact",
    });
  }

  return { groups, problems };
}

function codepointsOf(item: GlyphsSidebarItem, resolveGlyphName: GlyphNameResolver): number[] {
  const codepoints: number[] = [];

  if (item.infos) {
    for (const info of item.infos) {
      const hexValues =
        info.unicodes.length > 0 ? info.unicodes : info.unicode ? [info.unicode] : [];
      if (hexValues.length === 0 && info.name) {
        const resolved = codepointFromGlyphName(info.name) ?? resolveGlyphName(info.name);
        if (resolved !== null) codepoints.push(resolved);
      }

      for (const hex of hexValues) {
        const codepoint = Number.parseInt(hex, 16);
        if (Number.isInteger(codepoint) && codepoint <= 0x10ffff) codepoints.push(codepoint);
      }
    }
    return codepoints;
  }

  for (const name of item.coverage ?? []) {
    const resolved = codepointFromGlyphName(name) ?? resolveGlyphName(name);
    if (resolved !== null) codepoints.push(resolved);
  }
  return codepoints;
}

function slugify(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/(^-|-$)/g, "");
}
