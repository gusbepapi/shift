import MiniSearch from "minisearch";
import { alphabetical, comparing } from "@shift/types";
import { isEuropeanLanguageId } from "./europeanLanguages.js";
import { GlyphSets } from "./glyphSets.js";
import { createLanguageCatalog } from "./languageCatalog.js";
import { isConstructedLanguage } from "./languageKinds.js";
import { DEFAULT_SCRIPTS } from "./types.js";
import type {
  CharsetDefinition,
  CharsetSummary,
  CustomCategory,
  FontGlyph,
  GlyphCategory,
  GlyphCategoryCatalog,
  GlyphCategorySummary,
  GlyphSubCategorySummary,
  GlyphCodepointFilter,
  GlyphCategoryOptions,
  GlyphCodepointCategory,
  Glyph,
  GlyphInfoComponentResources,
  GlyphInfoCoreResources,
  GlyphInfoResources,
  Language,
  LanguageCatalog,
  LanguageGroup,
  LetterSkeleton,
  SearchResult,
  UnicodeProperties,
} from "./types.js";

const DEFAULT_GLYPH_CATEGORY_OPTIONS: Required<GlyphCategoryOptions> = {
  includeUnknown: true,
  unknownCategoryLabel: "Other",
  nullSubCategoryKey: "__null_subcategory__",
  nullSubCategoryLabel: "General",
};

const byLabel = comparing((summary: GlyphSubCategorySummary) => summary.label, alphabetical);
const byCategory = comparing((summary: GlyphCategorySummary) => summary.category, alphabetical);

interface GlyphSearchHit {
  id: number;
  codepoint: number;
  glyphName: string | null;
  unicodeName: string | null;
  category: GlyphCategory | null;
  subCategory: string | null;
  score: number;
}

function resolveGlyphCategoryOptions(
  options?: GlyphCategoryOptions,
): Required<GlyphCategoryOptions> {
  if (!options) {
    return DEFAULT_GLYPH_CATEGORY_OPTIONS;
  }

  return {
    includeUnknown: options?.includeUnknown ?? DEFAULT_GLYPH_CATEGORY_OPTIONS.includeUnknown,
    unknownCategoryLabel:
      options?.unknownCategoryLabel ?? DEFAULT_GLYPH_CATEGORY_OPTIONS.unknownCategoryLabel,
    nullSubCategoryKey:
      options?.nullSubCategoryKey ?? DEFAULT_GLYPH_CATEGORY_OPTIONS.nullSubCategoryKey,
    nullSubCategoryLabel:
      options?.nullSubCategoryLabel ?? DEFAULT_GLYPH_CATEGORY_OPTIONS.nullSubCategoryLabel,
  };
}

type GlyphCategoryEntry = {
  codepoint: number;
  category: string;
  subCategoryKey: string;
  /** Set for a membership of a custom category, which a glyph holds besides its own category. */
  overlay?: true;
};

type GlyphCategoryCountBucket = {
  count: number;
  subCategoryCounts: Map<string, { label: string; count: number }>;
};

type GlyphCategoryCatalogIndex = {
  categories: GlyphCategorySummary[];
  entries: GlyphCategoryEntry[];
};

function incrementCategoryCounts(
  categoryCounts: Map<string, GlyphCategoryCountBucket>,
  categoryInfo: GlyphCodepointCategory,
): void {
  let categoryBucket = categoryCounts.get(categoryInfo.category);
  if (!categoryBucket) {
    categoryBucket = { count: 0, subCategoryCounts: new Map() };
    categoryCounts.set(categoryInfo.category, categoryBucket);
  }
  categoryBucket.count += 1;

  const existingSubCategory = categoryBucket.subCategoryCounts.get(categoryInfo.subCategoryKey);
  if (existingSubCategory) {
    existingSubCategory.count += 1;
    return;
  }

  categoryBucket.subCategoryCounts.set(categoryInfo.subCategoryKey, {
    label: categoryInfo.subCategoryLabel,
    count: 1,
  });
}

function toSortedCategorySummaries(
  categoryCounts: Map<string, GlyphCategoryCountBucket>,
): GlyphCategorySummary[] {
  const summaries: GlyphCategorySummary[] = [];

  for (const [category, bucket] of categoryCounts.entries()) {
    const subCategories: GlyphCategorySummary["subCategories"] = [];
    for (const [key, subCategory] of bucket.subCategoryCounts.entries()) {
      subCategories.push({
        key,
        label: subCategory.label,
        count: subCategory.count,
      });
    }
    subCategories.sort(byLabel);

    summaries.push({
      category,
      count: bucket.count,
      subCategories,
    });
  }

  summaries.sort(byCategory);
  return summaries;
}

/**
 * In-memory Unicode glyph information database.
 *
 * Provides constant-time lookups for glyph metadata, codepoint decomposition,
 * charset membership, language coverage, and prefix-based full-text search
 * powered by MiniSearch.
 *
 * Construct with a {@link GlyphInfoResources} bundle. Call {@link close}
 * when done (currently a no-op, but present for future resource cleanup).
 */
export class GlyphInfo {
  #glyphData: Map<number, Glyph>;
  #glyphDataByName: Map<string, Glyph>;
  #glyphDataByAlias: Map<string, Glyph>;
  #decomposed: Map<number, number[]>;
  #usedBy: Map<number, number[]>;
  #charsets: CharsetDefinition[];
  #languages: Language[];
  #groups: LanguageGroup[];
  #skeletons: readonly LetterSkeleton[];
  #unicode: UnicodeProperties;
  #allGlyphData: readonly Glyph[];
  #glyphSets: GlyphSets | null = null;
  #searchData: Record<string, unknown>[] = [];
  #searchIndex: MiniSearch | null = null;
  #hasComponentResources = false;

  /**
   * @param resources - every resource, or only the core ones; until
   * {@link addResources} supplies the component resources, decomposition
   * lookups return nothing and {@link search} finds nothing.
   */
  constructor(resources: GlyphInfoResources | GlyphInfoCoreResources) {
    this.#allGlyphData = resources.glyphData;
    this.#glyphData = new Map(
      resources.glyphData.flatMap((g) => (g.codepoint === null ? [] : [[g.codepoint, g] as const])),
    );
    this.#glyphDataByName = new Map(resources.glyphData.map((g) => [g.name, g]));
    this.#glyphDataByAlias = new Map(this.#glyphDataByName);
    for (const glyph of resources.glyphData) {
      if (glyph.production && !this.#glyphDataByAlias.has(glyph.production)) {
        this.#glyphDataByAlias.set(glyph.production, glyph);
      }

      for (const alias of glyph.altNames?.split(",") ?? []) {
        const name = alias.trim();
        if (name && !this.#glyphDataByAlias.has(name)) {
          this.#glyphDataByAlias.set(name, glyph);
        }
      }
    }

    this.#decomposed = new Map();
    this.#usedBy = new Map();
    this.#charsets = resources.charsets;
    this.#languages = resources.languages;
    this.#groups = resources.groups;
    this.#skeletons = resources.skeletons;
    this.#unicode = resources.unicode;
    if ("searchData" in resources) this.addResources(resources);
  }

  /** Whether decomposition and search data are loaded. */
  get hasComponentResources(): boolean {
    return this.#hasComponentResources;
  }

  /** Supplies decomposition and search data to an instance built from core resources. */
  addResources(resources: GlyphInfoComponentResources): void {
    this.#decomposed = new Map(
      Object.entries(resources.decomposition.decomposed).map(([k, v]) => [Number(k), v]),
    );
    this.#usedBy = new Map(
      Object.entries(resources.decomposition.usedBy).map(([k, v]) => [Number(k), v]),
    );
    this.#searchData = resources.searchData;
    this.#searchIndex = null;
    this.#hasComponentResources = true;
  }

  #getSearchIndex(): MiniSearch {
    if (this.#searchIndex) return this.#searchIndex;

    const searchIndex = new MiniSearch({
      fields: ["glyphName", "unicodeName", "altNames", "category", "subCategory"],
      storeFields: ["codepoint", "glyphName", "unicodeName", "category", "subCategory"],
      idField: "codepoint",
      searchOptions: { prefix: true, combineWith: "AND" },
    });
    searchIndex.addAll(this.#searchData);
    this.#searchIndex = searchIndex;

    return searchIndex;
  }

  // Glyph Data (Map lookups)

  /** Look up the full metadata record for a codepoint, or `null` if unknown. */
  getGlyph(cp: number): Glyph | null {
    return this.#glyphData.get(cp) ?? null;
  }

  /** Shorthand for the production glyph name. Returns `null` if the codepoint is not in the database. */
  getGlyphName(cp: number): string | null {
    const data = this.#glyphData.get(cp);
    if (!data) return null;

    return data.name;
  }

  /**
   * Look up the full metadata record for a canonical glyph name, encoded or
   * not (`beh-ar.init` has no codepoint).
   */
  getGlyphByName(name: string): Glyph | null {
    return this.#glyphDataByName.get(name) ?? null;
  }

  /**
   * Resolve an authored glyph name to the canonical glyph database name.
   * Dot suffixes are preserved, and unknown names pass through unchanged.
   */
  resolveGlyphName(name: string, codepoint?: number | null): string {
    const suffixStart = name.indexOf(".");
    const baseName = suffixStart > 0 ? name.slice(0, suffixStart) : name;
    const suffix = suffixStart > 0 ? name.slice(suffixStart) : "";
    const glyph =
      (codepoint === null || codepoint === undefined ? null : this.#glyphData.get(codepoint)) ??
      this.#glyphDataByAlias.get(baseName);

    return glyph ? `${glyph.name}${suffix}` : name;
  }

  /**
   * Returns the codepoint of a glyph name, including its production name and alternative names.
   *
   * @returns `null` for an unknown name and for a name without a codepoint.
   */
  codepointForGlyphName(name: string): number | null {
    return this.#glyphDataByAlias.get(name)?.codepoint ?? null;
  }

  /** Every encoded glyph, by codepoint; unencoded forms are found by name. */
  getAllGlyph(): Glyph[] {
    return Array.from(this.#glyphData.values());
  }

  getGlyphCategories(): GlyphCategory[] {
    const categories = new Set<GlyphCategory>();
    for (const g of this.#glyphData.values()) {
      categories.add(g.category);
    }
    return Array.from(categories).sort();
  }

  getGlyphsByCategory(category: GlyphCategory): Glyph[] {
    const results: Glyph[] = [];
    for (const g of this.#glyphData.values()) {
      if (g.category === category) {
        results.push(g);
      }
    }
    return results;
  }

  /**
   * Resolve a codepoint into UI-friendly category metadata.
   * Unknown codepoints are assigned default labels from {@link GlyphCategoryOptions}.
   */
  getCategoryForCodepoint(cp: number, options?: GlyphCategoryOptions): GlyphCodepointCategory {
    return this.#getCategoryForCodepoint(cp, resolveGlyphCategoryOptions(options));
  }

  /**
   * Build a reusable category catalog for a codepoint set.
   * The returned `filter()` uses MiniSearch for query matching and preserves input order.
   *
   * @param customCategories - Categories the user defined; a glyph in one also stays in its own
   * category, and a custom category counts only the codepoints of the set.
   */
  createCategoryCatalog(
    codepoints: number[],
    options?: GlyphCategoryOptions,
    customCategories: readonly CustomCategory[] = [],
  ): GlyphCategoryCatalog {
    const resolvedOptions = resolveGlyphCategoryOptions(options);
    const data = this.#buildCategoryCatalogData(codepoints, resolvedOptions, customCategories);

    return {
      categories: data.categories,
      filter: (filter: GlyphCodepointFilter = {}) => this.#filterCategoryCatalog(data, filter),
    };
  }

  /**
   * Return grouped category/subcategory summaries for a codepoint set.
   */
  getCategorySummaries(
    codepoints: number[],
    options?: GlyphCategoryOptions,
  ): GlyphCategorySummary[] {
    return this.createCategoryCatalog(codepoints, options).categories;
  }

  /**
   * Filter a codepoint list by category/subcategory and optional full-text query.
   * Returned codepoints preserve input order.
   */
  filterCodepoints(
    codepoints: number[],
    filter: GlyphCodepointFilter = {},
    options?: GlyphCategoryOptions,
  ): number[] {
    const catalog = this.createCategoryCatalog(codepoints, options);
    return catalog.filter(filter);
  }

  // Decomposition (Map lookups)

  /**
   * Return the codepoints that `cp` decomposes into (e.g. an accented letter
   * into base + combining mark). Empty array if no decomposition exists.
   */
  getDecomposition(cp: number): number[] {
    return this.#decomposed.get(cp) ?? [];
  }

  /**
   * Return codepoints that use `cp` as a component in their decomposition
   * (the reverse of {@link getDecomposition}).
   */
  getUsedBy(cp: number): number[] {
    return this.#usedBy.get(cp) ?? [];
  }

  // Charsets (in-memory arrays)

  /** Return a summary (id, name, source, count) for every loaded charset. */
  listCharsets(): CharsetSummary[] {
    return this.#charsets.map(({ id, name, source, codepoints }) => ({
      id,
      name,
      source,
      count: codepoints.length,
    }));
  }

  /** Return the codepoints belonging to a charset, or an empty array if the ID is unknown. */
  getCharsetCodepoints(id: string): number[] {
    const charset = this.#charsets.find((c) => c.id === id);
    return charset?.codepoints ?? [];
  }

  // Languages

  /**
   * Identifiers a font tracks until it chooses its own: every group of the
   * {@link DEFAULT_SCRIPTS}, every Cyrillic language, every Latin-script language that needs more than the basic Latin
   * alphabet, every constructed language and every European language of another script.
   */
  defaultTrackedLanguageIds(): string[] {
    const scripts = new Set<string>(DEFAULT_SCRIPTS);
    const basic = new Set(
      this.#groups.find(({ id }) => id === "group-latin-basic")?.baseCodepoints,
    );
    // Hyperglot lists Æ and Œ for English loanwords, which English does not need to be set
    const loanwordLigatures = new Set([0xc6, 0xe6, 0x152, 0x153]);
    const needsMoreThanBasic = (language: Language): boolean =>
      language.baseCodepoints.some(
        (codepoint) => !basic.has(codepoint) && !loanwordLigatures.has(codepoint),
      );

    const groupIds = this.#groups.filter(({ script }) => scripts.has(script)).map(({ id }) => id);
    const languageIds = this.#languages
      .filter((language) => {
        if (isConstructedLanguage(language)) return language.orthography === "primary";
        if (language.orthography !== "primary") return false;
        if (language.script === "Cyrillic") return true;
        if (language.script === "Latin") return basic.size === 0 || needsMoreThanBasic(language);
        return isEuropeanLanguageId(language.id);
      })
      .map(({ id }) => id);

    return [...groupIds, ...languageIds];
  }

  /**
   * Builds language, script-group and character-set coverage for a font, by glyph name.
   *
   * @param glyphs - the font's glyphs; one counts for a required glyph when it
   * encodes the same character or its name resolves to the same GlyphData name.
   * @param trackedLanguageIds - languages and character sets listed in
   * {@link LanguageCatalog.trackedScripts}; a tracked language's script also gets its groups.
   * @param customGroups - Character sets the user defined; they list after the built-in ones of
   * their script and replace a built-in one with the same identifier.
   * @returns A catalog whose filter preserves the input glyph order.
   */
  createLanguageCatalog(
    glyphs: readonly FontGlyph[],
    trackedLanguageIds: readonly string[] = [],
    customGroups: readonly LanguageGroup[] = [],
  ): LanguageCatalog {
    this.#glyphSets ??= new GlyphSets(
      this.#allGlyphData,
      (name) => this.#glyphDataByAlias.get(name)?.codepoint ?? null,
      (codepoint) => this.getGlyphName(codepoint) ?? fallbackGlyphName(codepoint),
      this.#skeletons,
      this.#unicode,
    );
    const replaced = new Set(customGroups.map(({ id }) => id));
    const characterSets = [...this.#groups.filter(({ id }) => !replaced.has(id)), ...customGroups];

    return createLanguageCatalog(
      this.#glyphSets,
      this.#languages,
      characterSets,
      glyphs,
      trackedLanguageIds,
      (name, codepoint) => this.resolveGlyphName(name, codepoint),
    );
  }

  // Search (MiniSearch)

  /**
   * Full-text prefix search across glyph names, Unicode names, and categories.
   * @param query Free-text query string. Special characters are stripped.
   * @param limit Maximum number of results (default 50).
   * @returns Results ranked by relevance score.
   */
  search(query: string, limit = 50): SearchResult[] {
    if (!query.trim()) return [];

    const sanitized = query.trim().replace(/['"()*:^{}]/g, " ");
    if (!sanitized.trim()) return [];

    const results = this.#getSearchIndex().search(sanitized, {
      prefix: true,
      combineWith: "AND",
    }) as unknown as GlyphSearchHit[];

    return results.slice(0, limit).map((result) => ({
      codepoint: result.codepoint ?? result.id,
      glyphName: result.glyphName ?? null,
      unicodeName: result.unicodeName ?? null,
      category: result.category ?? null,
      subCategory: result.subCategory ?? null,
      rank: result.score,
    }));
  }

  /** Release resources. Currently a no-op (MiniSearch is in-memory). */
  close(): void {
    // No-op — MiniSearch is in-memory, no resources to release
  }

  #getCategoryForCodepoint(
    codepoint: number,
    options: Required<GlyphCategoryOptions>,
  ): GlyphCodepointCategory {
    const glyphData = this.#glyphData.get(codepoint);
    if (!glyphData) {
      return {
        category: options.unknownCategoryLabel as GlyphCategory,
        subCategoryKey: options.nullSubCategoryKey,
        subCategoryLabel: options.nullSubCategoryLabel,
        isKnown: false,
      };
    }

    return {
      category: glyphData.category,
      subCategoryKey: glyphData.subCategory ?? options.nullSubCategoryKey,
      subCategoryLabel: glyphData.subCategory ?? options.nullSubCategoryLabel,
      isKnown: true,
    };
  }

  #buildCategoryCatalogData(
    codepoints: number[],
    options: Required<GlyphCategoryOptions>,
    customCategories: readonly CustomCategory[],
  ): GlyphCategoryCatalogIndex {
    const categoryCounts = new Map<string, GlyphCategoryCountBucket>();
    const entries: GlyphCategoryEntry[] = [];

    for (const codepoint of codepoints) {
      const categoryInfo = this.#getCategoryForCodepoint(codepoint, options);
      if (!options.includeUnknown && !categoryInfo.isKnown) {
        continue;
      }

      const { category, subCategoryKey } = categoryInfo;
      entries.push({ codepoint, category, subCategoryKey });
      incrementCategoryCounts(categoryCounts, categoryInfo);
    }

    const present = new Set(codepoints);
    for (const custom of customCategories) {
      for (const subCategory of custom.subCategories) {
        for (const codepoint of subCategory.codepoints) {
          if (!present.has(codepoint)) continue;

          entries.push({
            codepoint,
            category: custom.name,
            subCategoryKey: subCategory.name,
            overlay: true,
          });
          incrementCategoryCounts(categoryCounts, {
            category: custom.name,
            subCategoryKey: subCategory.name,
            subCategoryLabel: subCategory.name,
            isKnown: true,
          });
        }
      }
    }

    return {
      categories: toSortedCategorySummaries(categoryCounts),
      entries,
    };
  }

  #filterCategoryCatalog(data: GlyphCategoryCatalogIndex, filter: GlyphCodepointFilter): number[] {
    const selectedCategory = filter.category ?? null;
    const selectedSubCategoryKey = filter.subCategoryKey ?? null;
    const query = filter.query ?? "";

    if (selectedSubCategoryKey !== null && selectedCategory === null) {
      return [];
    }

    const trimmedQuery = query.trim();
    let searchMatches: Set<number> | null = null;
    if (trimmedQuery.length > 0) {
      const searchLimit = filter.searchLimit ?? Math.max(data.entries.length, 200);
      const searchResults = this.search(trimmedQuery, searchLimit);
      searchMatches = new Set(searchResults.map((result) => result.codepoint));
    }

    const filtered: number[] = [];
    for (const entry of data.entries) {
      if (entry.overlay && selectedCategory === null) {
        continue;
      }
      if (selectedCategory !== null && entry.category !== selectedCategory) {
        continue;
      }
      if (selectedSubCategoryKey !== null && entry.subCategoryKey !== selectedSubCategoryKey) {
        continue;
      }
      if (searchMatches !== null && !searchMatches.has(entry.codepoint)) {
        continue;
      }
      filtered.push(entry.codepoint);
    }

    return filtered;
  }
}

/** The `uniXXXX` / `uXXXXX` name of a character GlyphData does not name. */
function fallbackGlyphName(codepoint: number): string {
  const hex = codepoint.toString(16).toUpperCase();
  return codepoint > 0xffff ? `u${hex}` : `uni${hex.padStart(4, "0")}`;
}
