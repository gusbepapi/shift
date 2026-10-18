// TODO: Derive GlyphSubCategory and GlyphScript unions from GlyphData.xml during the generate step.

/** Known Unicode general categories used for glyph classification. */
export const GLYPH_CATEGORIES = [
  "Letter",
  "Mark",
  "Number",
  "Punctuation",
  "Separator",
  "Symbol",
  "Other",
] as const;

export type GlyphCategory = (typeof GLYPH_CATEGORIES)[number];

export interface Glyph {
  codepoint: number;
  name: string;
  category: GlyphCategory;
  subCategory: string | null; // TODO: narrow to GlyphSubCategory union
  script: string | null; // TODO: narrow to GlyphScript union
  production: string | null;
  altNames: string | null;
}

export interface CharsetDefinition {
  id: string; // TODO: narrow to CharsetId union once more charsets are added (e.g. "adobe-latin-1" | "adobe-latin-2" | ...)
  name: string;
  source: string; // TODO: narrow to CharsetSource union (e.g. "adobe" | "google" | "custom")
  codepoints: number[];
}

export interface CharsetSummary {
  id: string;
  name: string;
  source: string;
  count: number;
}

/** Whether the language is spoken today, constructed, or no longer spoken. */
export type LanguageStatus = "living" | "constructed" | "historical";

/** How far Hyperglot has reviewed the orthography data; `draft` is the least reviewed. */
export type LanguageValidity = "todo" | "draft" | "preliminary" | "verified";

/** Whether the orthography is the language's main one or an additional one. */
export type LanguageOrthography = "primary" | "secondary";

/** Represents a Hyperglot orthography and its required base-character repertoire. */
export interface Language {
  id: string;
  name: string;
  autonym: string | null;
  script: string;
  baseCodepoints: number[];
  status: LanguageStatus;
  validity: LanguageValidity;
  orthography: LanguageOrthography;
}

export interface LanguageCoverage {
  language: Language;
  presentCount: number;
  requiredCount: number;
}

/**
 * A named set of characters listed under a script, in the manner of the Glyphs
 * sidebar, for example «Western European» under Latin.
 */
export interface LanguageGroup {
  id: string;
  name: string;
  script: string;
  /** Heading the group sits under inside its script, for example «Character Sets»; `null` for none. */
  section: string | null;
  baseCodepoints: number[];
  /**
   * `exact` for a published encoding or a list read from Glyphs itself, `derived` for a set
   * assembled from Hyperglot or GlyphData, and `custom` for a group the user defined.
   */
  basis: "exact" | "derived" | "custom";
}

export interface LanguageGroupCoverage {
  group: LanguageGroup;
  presentCount: number;
  requiredCount: number;
}

export interface LanguageScript {
  script: string;
  /** Glyphs-style groups, in their declared order. */
  groups: LanguageGroupCoverage[];
  languages: LanguageCoverage[];
}

/**
 * Groups tracked by a font that has not chosen its own list: the basic sets of the
 * three scripts most fonts begin with. The picker adds every other script.
 */
export const DEFAULT_GROUP_IDS = [
  "group-latin-basic",
  "group-latin-western-european",
  "group-cyrillic-basic",
  "group-greek-basic",
] as const;

/**
 * Languages tracked by a font that has not chosen its own list: the ten most
 * spoken languages by total speakers (Ethnologue), as Hyperglot orthography ids.
 */
export const DEFAULT_LANGUAGE_IDS = [
  "eng-latin",
  "cmn-chinese",
  "hin-devanagari",
  "spa-latin",
  "fra-latin",
  "arb-arabic",
  "ben-bengali",
  "por-latin",
  "rus-cyrillic",
  "urd-arabic",
] as const;

export interface LanguageCatalog {
  /** Every known language, grouped by script. */
  scripts: LanguageScript[];
  /** Returns the font's codepoints that the language or group requires, in input order. */
  filter(languageId: string): number[];
  /** Returns coverage for one language, or `null` for an unknown id. */
  coverage(languageId: string): LanguageCoverage | null;
  /** Returns coverage for one group, or `null` for an unknown id. */
  groupCoverage(groupId: string): LanguageGroupCoverage | null;
  /** Returns every codepoint the language or group requires, ascending. */
  required(languageId: string): number[];
  /** Groups the given languages and groups by script, skipping unknown ids. */
  scriptsFor(languageIds: readonly string[]): LanguageScript[];
}

export interface SearchResult {
  codepoint: number;
  glyphName: string | null;
  unicodeName: string | null;
  category: GlyphCategory | null;
  subCategory: string | null;
  rank: number;
}

export interface GlyphCategoryOptions {
  includeUnknown?: boolean;
  unknownCategoryLabel?: string;
  nullSubCategoryKey?: string;
  nullSubCategoryLabel?: string;
}

export interface GlyphCodepointCategory {
  category: GlyphCategory;
  subCategoryKey: string;
  subCategoryLabel: string;
  isKnown: boolean;
}

export interface GlyphSubCategorySummary {
  key: string;
  label: string;
  count: number;
}

export interface GlyphCategorySummary {
  category: GlyphCategory;
  count: number;
  subCategories: GlyphSubCategorySummary[];
}

export interface GlyphCodepointFilter {
  category?: GlyphCategory | null;
  subCategoryKey?: string | null;
  query?: string;
  searchLimit?: number;
}

export interface GlyphCategoryCatalog {
  categories: GlyphCategorySummary[];
  filter(filter?: GlyphCodepointFilter): number[];
}

export interface Decomposition {
  decomposed: Record<string, number[]>;
  usedBy: Record<string, number[]>;
}

export interface GlyphInfoResources extends GlyphInfoCoreResources, GlyphInfoComponentResources {}

/** Names, categories, charsets, and languages: what glyph lists and catalogs need. */
export interface GlyphInfoCoreResources {
  glyphData: Glyph[];
  charsets: CharsetDefinition[];
  languages: Language[];
  groups: LanguageGroup[];
}

/**
 * Decomposition and full-text search data, used to suggest and find components.
 *
 * About 10 MB of the 17 MB dataset; apps that start with the core resources
 * load these when they first need them and pass them to `GlyphInfo.addResources`.
 */
export interface GlyphInfoComponentResources {
  decomposition: Decomposition;
  searchData: Record<string, unknown>[];
}
