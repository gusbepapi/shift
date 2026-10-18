import { parseCustomGroups, parseGlyphsGroupsPlist } from "@shift/glyph-info";
import type {
  CustomCategory,
  CustomGroupsResult,
  GlyphInfo,
  LanguageGroup,
} from "@shift/glyph-info";
import type { LanguageGroupFile } from "@shared/config/config";

/** What reading the custom language group files produced: the groups and a line for each problem. */
export interface CustomLanguageGroups {
  groups: LanguageGroup[];
  /** Catalogue categories, such as «Ancient writing systems», read from the same files. */
  categories: CustomCategory[];
  problems: string[];
}

/**
 * Reads the files of `language-groups/` into groups for the language catalog.
 *
 * @remarks
 * A file with problems still contributes its valid groups. When two files define the same group
 * identifier, the later file by name wins.
 *
 * @param glyphInfo - Resolves glyph names such as `aacute` in a Glyphs `Groups.plist`.
 * @param files - The files in name order.
 */
export function readCustomLanguageGroups(
  glyphInfo: Pick<GlyphInfo, "codepointForGlyphName">,
  files: readonly LanguageGroupFile[],
): CustomLanguageGroups {
  const byId = new Map<string, LanguageGroup>();
  const categories = new Map<string, Map<string, CustomCategory["subCategories"][number]>>();
  const problems: string[] = [];

  for (const file of files) {
    const result: CustomGroupsResult =
      file.format === "json"
        ? parseCustomGroups(file.text)
        : parseGlyphsGroupsPlist(file.text, (name) => glyphInfo.codepointForGlyphName(name));

    for (const group of result.groups) byId.set(group.id, group);
    for (const category of result.categories) {
      const subCategories = categories.get(category.name) ?? new Map();
      for (const subCategory of category.subCategories) {
        subCategories.set(subCategory.name, subCategory);
      }
      categories.set(category.name, subCategories);
    }
    for (const problem of result.problems) problems.push(`${file.name}: ${problem}`);
  }

  return {
    groups: [...byId.values()],
    categories: [...categories].map(([name, subCategories]) => ({
      name,
      subCategories: [...subCategories.values()],
    })),
    problems,
  };
}
