/**
 * `@shift/glyph-info` -- Unicode glyph metadata, decomposition, charset and
 * language coverage, and full-text search.
 *
 * Instantiate {@link GlyphInfo} with a {@link GlyphInfoResources} bundle
 * (produced by the `generate:glyph-info` script) to get fast, in-memory
 * lookups. Use {@link defaultResources} for the pre-built dataset shipped
 * with the package.
 *
 * ```ts
 * import { GlyphInfo, defaultResources } from "@shift/glyph-info";
 * const info = new GlyphInfo(defaultResources);
 * info.search("latin capital");
 * ```
 *
 * @packageDocumentation
 */

export { GlyphInfo } from "./GlyphInfo.js";

/** Load the default pre-built resource bundle. */
export { defaultResources } from "./resources.js";

/** The pre-built core resources alone, for apps that load component data later. */
export { coreResources } from "./coreResources.js";

export { DEFAULT_SCRIPTS, DEFAULT_LANGUAGE_IDS, GLYPH_CATEGORIES } from "./types.js";

/** Readers for groups a user defines, in Shift's JSON format or as a Glyphs `Groups.plist`. */
export { parseCustomGroups, parseGlyphsGroupsPlist } from "./customGroups.js";
export type { CustomGroupsResult, GlyphNameResolver } from "./customGroups.js";
export type {
  /** Full metadata record for a single Unicode codepoint. */
  CharsetDefinition,
  /** Lightweight summary returned by `GlyphInfo.listCharsets()`. */
  CharsetSummary,
  /** Bidirectional codepoint decomposition maps. */
  Decomposition,
  /** Known Unicode general category for glyph classification. */
  CustomCategory,
  GlyphCategory,
  /** Category fallback and inclusion options for codepoint categorization APIs. */
  GlyphCategoryOptions,
  /** Reusable category catalog with summaries and filtering. */
  GlyphCategoryCatalog,
  /** Resolved category/subcategory metadata for an individual codepoint. */
  GlyphCodepointCategory,
  /** Filter arguments used by `GlyphInfo.filterCodepoints()`. */
  GlyphCodepointFilter,
  /** Category summary with subcategory counts. */
  GlyphCategorySummary,
  /** Subcategory summary entry used inside `GlyphCategorySummary`. */
  GlyphSubCategorySummary,
  /** Per-codepoint glyph metadata (name, category, script, etc.). */
  Glyph,
  /** Bundle of all resources needed to construct a {@link GlyphInfo} instance. */
  GlyphInfoResources,
  /** Names, categories, charsets, and languages. */
  GlyphInfoCoreResources,
  /** Decomposition and search data, loadable after construction. */
  GlyphInfoComponentResources,
  /** Represents a language orthography and its required base-character repertoire. */
  Language,
  /** Provides reusable language coverage grouped by script. */
  LanguageCatalog,
  /** Whether a language orthography is primary or secondary. */
  LanguageOrthography,
  /** Whether a language is living, constructed, or historical. */
  LanguageStatus,
  /** How far Hyperglot has reviewed a language orthography. */
  LanguageValidity,
  /** Reports coverage of one language orthography within a font. */
  LanguageCoverage,
  /** A Glyphs-style named character set listed under a script. */
  LanguageGroup,
  /** Reports coverage of one group within a font. */
  LanguageGroupCoverage,
  /** Groups language coverage entries sharing a script. */
  LanguageScript,
  /** A single hit from `GlyphInfo.search()`, ranked by relevance. */
  SearchResult,
} from "./types.js";
export { EUROPEAN_LANGUAGE_CODES, isEuropeanLanguageId } from "./europeanLanguages.js";
export { isConstructedLanguage } from "./languageKinds.js";
