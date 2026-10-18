import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { GlyphInfo } from "./GlyphInfo.js";
import {
  DEFAULT_SCRIPTS,
  DEFAULT_LANGUAGE_IDS,
  type GlyphCategory,
  type LanguageGroup,
} from "./types.js";
import { isEuropeanLanguageId } from "./europeanLanguages.js";
import { defaultResources } from "./resources.js";
import { coreResources } from "./coreResources.js";
import { componentResources } from "./componentResources.js";

let db: GlyphInfo;

beforeAll(() => {
  db = new GlyphInfo(defaultResources);
});

afterAll(() => {
  db.close();
});

describe("getGlyph", () => {
  it("returns full data for dollar sign", () => {
    const result = db.getGlyph(0x24);
    expect(result).toEqual({
      codepoint: 0x24,
      name: "dollar",
      category: "Symbol",
      subCategory: "Currency",
      script: null,
      production: null,
      altNames: null,
    });
  });

  it("returns data for LATIN CAPITAL LETTER A", () => {
    const result = db.getGlyph(0x41);
    expect(result).not.toBeNull();
    expect(result!.name).toBe("A");
    expect(result!.category).toBe("Letter");
    expect(result!.script).toBe("latin");
  });

  it("returns null for codepoint not in Glyph.xml", () => {
    expect(db.getGlyph(0xfffe)).toBeNull();
  });

  it("returns data for space (U+0020)", () => {
    const result = db.getGlyph(0x20);
    expect(result).not.toBeNull();
    expect(result!.name).toBe("space");
    expect(result!.category).toBe("Separator");
    expect(result!.subCategory).toBe("Space");
  });

  it("returns data with altNames field", () => {
    // bar (U+007C) has altNames="verticalbar"
    const result = db.getGlyph(0x7c);
    expect(result).not.toBeNull();
    expect(result!.name).toBe("bar");
    expect(result!.altNames).toBe("verticalbar");
  });

  it("returns data with production field", () => {
    // nbspace (U+00A0) has production="uni00A0"
    const result = db.getGlyph(0xa0);
    expect(result).not.toBeNull();
    expect(result!.production).toBe("uni00A0");
  });

  it("returns null for negative codepoint", () => {
    expect(db.getGlyph(-1)).toBeNull();
  });

  it("returns null for codepoint beyond BMP", () => {
    expect(db.getGlyph(0x10000)).toBeNull();
  });

  it("returns null for surrogate codepoint", () => {
    expect(db.getGlyph(0xd800)).toBeNull();
  });

  it("returns data for digit zero", () => {
    const result = db.getGlyph(0x30);
    expect(result).not.toBeNull();
    expect(result!.name).toBe("zero");
    expect(result!.category).toBe("Number");
    expect(result!.subCategory).toBe("Decimal Digit");
  });
});

describe("getGlyphName", () => {
  it("returns name for dollar sign", () => {
    expect(db.getGlyphName(0x24)).toBe("dollar");
  });

  it("returns null for unknown codepoint", () => {
    expect(db.getGlyphName(0xfffe)).toBeNull();
  });

  it("returns name for lowercase letters", () => {
    expect(db.getGlyphName(0x61)).toBe("a");
    expect(db.getGlyphName(0x7a)).toBe("z");
  });

  it("returns name for punctuation", () => {
    expect(db.getGlyphName(0x2e)).toBe("period");
    expect(db.getGlyphName(0x2c)).toBe("comma");
  });
});

describe("resolveGlyphName", () => {
  it("maps legacy aliases from the glyph database", () => {
    expect(db.resolveGlyphName("afii10072")).toBe("zhe-cy");
    expect(db.resolveGlyphName("afii10081")).toBe("pe-cy");
  });

  it("uses the assigned codepoint when the authored name is noncanonical", () => {
    expect(db.resolveGlyphName("custom-zhe", 0x0436)).toBe("zhe-cy");
  });

  it("maps production names and preserves dot suffixes", () => {
    expect(db.resolveGlyphName("uni01C9.sc")).toBe("lj.sc");
  });

  it("preserves unknown authored names", () => {
    expect(db.resolveGlyphName("custom.unencoded")).toBe("custom.unencoded");
  });
});

describe("getAllGlyph", () => {
  it("returns many entries", () => {
    const all = db.getAllGlyph();
    expect(all.length).toBeGreaterThan(100);
  });

  it("every entry has required fields", () => {
    const all = db.getAllGlyph();
    for (const g of all) {
      expect(g.codepoint).toBeTypeOf("number");
      expect(g.name).toBeTypeOf("string");
      expect(g.name.length).toBeGreaterThan(0);
      expect(g.category).toBeTypeOf("string");
      expect(g.category.length).toBeGreaterThan(0);
    }
  });

  it("contains no duplicate codepoints", () => {
    const all = db.getAllGlyph();
    const codepoints = all.map((g) => g.codepoint);
    const uniqueCodepoints = new Set(codepoints);
    expect(uniqueCodepoints.size).toBe(codepoints.length);
  });

  it("codepoints are all non-negative integers", () => {
    const all = db.getAllGlyph();
    for (const g of all) {
      expect(g.codepoint).toBeGreaterThanOrEqual(0);
      expect(Number.isInteger(g.codepoint)).toBe(true);
    }
  });
});

describe("getGlyphCategories", () => {
  it("returns sorted unique categories", () => {
    const categories = db.getGlyphCategories();
    expect(categories.length).toBeGreaterThan(0);
    expect(categories).toContain("Letter");
    expect(categories).toContain("Symbol");
    const sorted = [...categories].sort();
    expect(categories).toEqual(sorted);
  });

  it("contains no duplicates", () => {
    const categories = db.getGlyphCategories();
    expect(new Set(categories).size).toBe(categories.length);
  });

  it("includes common font editing categories", () => {
    const categories = db.getGlyphCategories();
    expect(categories).toContain("Number");
    expect(categories).toContain("Punctuation");
    expect(categories).toContain("Separator");
  });
});

describe("getGlyphsByCategory", () => {
  it("returns glyphs in the Letter category", () => {
    const letters = db.getGlyphsByCategory("Letter");
    expect(letters.length).toBeGreaterThan(0);
    expect(letters.every((g) => g.category === "Letter")).toBe(true);
  });

  it("returns empty array for non-existent category", () => {
    expect(db.getGlyphsByCategory("NonExistent" as GlyphCategory)).toEqual([]);
  });

  it("all returned glyphs match the requested category", () => {
    const symbols = db.getGlyphsByCategory("Symbol");
    for (const g of symbols) {
      expect(g.category).toBe("Symbol");
    }
  });

  it("category results are consistent with getAllGlyph", () => {
    const all = db.getAllGlyph();
    const categories = db.getGlyphCategories();
    let totalFromCategories = 0;
    for (const cat of categories) {
      totalFromCategories += db.getGlyphsByCategory(cat).length;
    }
    expect(totalFromCategories).toBe(all.length);
  });
});

describe("getDecomposition", () => {
  it("decomposes é (U+00E9) to e + combining acute", () => {
    const components = db.getDecomposition(0xe9);
    expect(components).toEqual([0x65, 0x301]);
  });

  it("returns empty array for a base character", () => {
    expect(db.getDecomposition(0x41)).toEqual([]);
  });

  it("decomposes ñ (U+00F1) to n + combining tilde", () => {
    const components = db.getDecomposition(0xf1);
    expect(components).toEqual([0x6e, 0x303]);
  });

  it("decomposes ü (U+00FC) to u + combining diaeresis", () => {
    const components = db.getDecomposition(0xfc);
    expect(components).toEqual([0x75, 0x308]);
  });

  it("returns empty for surrogate codepoint", () => {
    expect(db.getDecomposition(0xd800)).toEqual([]);
  });

  it("returns empty for negative codepoint", () => {
    expect(db.getDecomposition(-1)).toEqual([]);
  });

  it("decomposes fi ligature (U+FB01) into f + i", () => {
    const components = db.getDecomposition(0xfb01);
    expect(components).toContain(0x66); // f
    expect(components).toContain(0x69); // i
  });

  it("decomposes superscript 2 (U+00B2) to digit 2", () => {
    const components = db.getDecomposition(0xb2);
    expect(components).toContain(0x32); // 2
  });

  it("decomposition components are all valid codepoints", () => {
    // Spot-check several decomposable characters
    for (const cp of [0xe9, 0xf1, 0xfc, 0xc0, 0xd6]) {
      const components = db.getDecomposition(cp);
      for (const c of components) {
        expect(c).toBeGreaterThanOrEqual(0);
        expect(c).toBeLessThanOrEqual(0xffff);
      }
    }
  });
});

describe("getUsedBy", () => {
  it("returns composed characters using e (U+0065)", () => {
    const usedBy = db.getUsedBy(0x65);
    expect(usedBy.length).toBeGreaterThan(0);
    expect(usedBy).toContain(0xe9); // é
    expect(usedBy).toContain(0xe8); // è
    expect(usedBy).toContain(0xea); // ê
    expect(usedBy).toContain(0xeb); // ë
  });

  it("returns empty array for a character not used as component", () => {
    expect(db.getUsedBy(0xfffe)).toEqual([]);
  });

  it("usedBy is sorted in ascending order", () => {
    const usedBy = db.getUsedBy(0x65);
    for (let i = 1; i < usedBy.length; i++) {
      expect(usedBy[i]).toBeGreaterThan(usedBy[i - 1]);
    }
  });

  it("combining acute (U+0301) is used by many accented characters", () => {
    const usedBy = db.getUsedBy(0x301);
    expect(usedBy.length).toBeGreaterThan(10);
    expect(usedBy).toContain(0xe9); // é
    expect(usedBy).toContain(0xc1); // Á
  });

  it("decomposition and usedBy are consistent", () => {
    // If é decomposes to [e, acute], then usedBy(e) should contain é
    const components = db.getDecomposition(0xe9);
    for (const comp of components) {
      const usedBy = db.getUsedBy(comp);
      expect(usedBy).toContain(0xe9);
    }
  });

  it("returns empty for negative codepoint", () => {
    expect(db.getUsedBy(-1)).toEqual([]);
  });
});

describe("listCharsets", () => {
  it("includes adobe-latin-1", () => {
    const charsets = db.listCharsets();
    expect(charsets.length).toBeGreaterThan(0);
    const al1 = charsets.find((c) => c.id === "adobe-latin-1");
    expect(al1).toBeDefined();
    expect(al1!.name).toBe("Adobe Latin 1");
    expect(al1!.source).toBe("adobe");
    expect(al1!.count).toBeGreaterThan(200);
  });

  it("summary count matches codepoints array length", () => {
    const charsets = db.listCharsets();
    for (const cs of charsets) {
      const codepoints = db.getCharsetCodepoints(cs.id);
      expect(cs.count).toBe(codepoints.length);
    }
  });

  it("does not expose codepoints array in summary", () => {
    const charsets = db.listCharsets();
    for (const cs of charsets) {
      expect(cs).not.toHaveProperty("codepoints");
    }
  });
});

describe("getCharsetCodepoints", () => {
  it("returns codepoints for adobe-latin-1", () => {
    const codepoints = db.getCharsetCodepoints("adobe-latin-1");
    expect(codepoints.length).toBeGreaterThan(200);
    expect(codepoints).toContain(0x41); // A
    expect(codepoints).toContain(0x24); // $
  });

  it("returns empty for unknown charset", () => {
    expect(db.getCharsetCodepoints("nonexistent")).toEqual([]);
  });

  it("contains basic ASCII letters and digits", () => {
    const codepoints = db.getCharsetCodepoints("adobe-latin-1");
    // A-Z
    for (let cp = 0x41; cp <= 0x5a; cp++) {
      expect(codepoints).toContain(cp);
    }
    // a-z
    for (let cp = 0x61; cp <= 0x7a; cp++) {
      expect(codepoints).toContain(cp);
    }
    // 0-9
    for (let cp = 0x30; cp <= 0x39; cp++) {
      expect(codepoints).toContain(cp);
    }
  });

  it("contains common accented characters", () => {
    const codepoints = db.getCharsetCodepoints("adobe-latin-1");
    expect(codepoints).toContain(0xe9); // é
    expect(codepoints).toContain(0xf1); // ñ
    expect(codepoints).toContain(0xfc); // ü
    expect(codepoints).toContain(0xe7); // ç
  });

  it("contains fi and fl ligatures", () => {
    const codepoints = db.getCharsetCodepoints("adobe-latin-1");
    expect(codepoints).toContain(0xfb01); // fi
    expect(codepoints).toContain(0xfb02); // fl
  });

  it("contains euro sign", () => {
    const codepoints = db.getCharsetCodepoints("adobe-latin-1");
    expect(codepoints).toContain(0x20ac);
  });

  it("codepoints are all valid non-negative integers", () => {
    const codepoints = db.getCharsetCodepoints("adobe-latin-1");
    for (const cp of codepoints) {
      expect(cp).toBeGreaterThanOrEqual(0);
      expect(Number.isInteger(cp)).toBe(true);
    }
  });

  it("returns empty string for empty id", () => {
    expect(db.getCharsetCodepoints("")).toEqual([]);
  });
});

describe("language coverage catalogs", () => {
  it("reports required base-character coverage for primary orthographies", () => {
    const catalog = db.createLanguageCatalog([0x41, 0x42, 0x61, 0x24]);
    const latin = catalog.scripts.find((script) => script.script === "Latin");
    const english = latin?.languages.find(({ language }) => language.id === "eng-latin");

    expect(english?.language.name).toBe("English");
    expect(english?.presentCount).toBe(3);
    expect(english?.requiredCount).toBe(56);
  });

  it("filters the font repertoire by language while preserving input order", () => {
    const catalog = db.createLanguageCatalog([0x42, 0x24, 0x41, 0x61]);

    expect(catalog.filter("eng-latin")).toEqual([0x42, 0x41, 0x61]);
    expect(catalog.filter("unknown-language")).toEqual([]);
  });

  it("lists every required base character in codepoint order", () => {
    const catalog = db.createLanguageCatalog([0x61, 0x62]);
    const required = catalog.required("eng-latin");

    expect(required).toHaveLength(56);
    expect(required).toContain(0x61);
    expect(required).toEqual([...required].sort((left, right) => left - right));
    expect(catalog.required("unknown-language")).toEqual([]);
  });

  it("groups tracked languages by script and skips unknown ids", () => {
    const catalog = db.createLanguageCatalog([]);
    const scripts = catalog.scriptsFor(["rus-cyrillic", "eng-latin", "fra-latin", "nope"]);

    expect(scripts.map(({ script }) => script)).toEqual(["Cyrillic", "Latin"]);
    expect(scripts[1]?.languages.map(({ language }) => language.id)).toEqual([
      "eng-latin",
      "fra-latin",
    ]);
  });

  it("resolves every default language id", () => {
    const catalog = db.createLanguageCatalog([]);

    for (const languageId of DEFAULT_LANGUAGE_IDS) {
      expect(catalog.coverage(languageId)?.language.id).toBe(languageId);
    }
  });

  it("includes historical languages with their status", () => {
    const catalog = db.createLanguageCatalog([]);
    const akkadian = catalog.coverage("akk-sumero-akkadian-cuneiforms")?.language;

    expect(akkadian?.name).toBe("Akkadian");
    expect(akkadian?.status).toBe("historical");
    expect(catalog.scripts.map(({ script }) => script)).toContain("Egyptian Hieroglyphs");
  });

  it("includes draft data and marks it as such", () => {
    const catalog = db.createLanguageCatalog([]);
    const aimaq = catalog.coverage("aiq-arabic")?.language;

    expect(aimaq?.validity).toBe("draft");
    expect(aimaq?.orthography).toBe("primary");
  });

  it("lists secondary orthographies under their own identifier", () => {
    const catalog = db.createLanguageCatalog([]);
    const secondary = catalog.coverage("mey-arabic-secondary")?.language;

    expect(secondary?.orthography).toBe("secondary");
    expect(catalog.coverage("mey-arabic")).toBeNull();
  });

  it("keeps identifiers unique and keeps primary identifiers free of status suffixes", () => {
    const catalog = db.createLanguageCatalog([]);
    const languages = catalog.scripts.flatMap(({ languages }) => languages.map((c) => c.language));
    const identifiers = languages.map(({ id }) => id);

    expect(new Set(identifiers).size).toBe(identifiers.length);
    for (const language of languages) {
      expect(/-secondary(-\d+)?$/.test(language.id)).toBe(language.orthography === "secondary");
    }
  });

  it("lists Glyphs-style groups under their scripts in declared order", () => {
    const catalog = db.createLanguageCatalog([]);
    const latin = catalog.scripts.find((script) => script.script === "Latin");
    const names = latin?.groups.map(({ group }) => group.name);

    expect(names?.slice(0, 3)).toEqual(["Basic", "Western European", "Central European"]);
    expect(names).toContain("Pan African");
    expect(names).toContain("Windows 1252");
  });

  it("builds the Windows 1252 character set exactly", () => {
    const catalog = db.createLanguageCatalog([0x20ac, 0x41]);
    const windows = catalog.groupCoverage("group-latin-windows-1252");

    expect(windows?.group.section).toBe("Character Sets");
    expect(windows?.group.basis).toBe("exact");
    expect(windows?.requiredCount).toBe(218);
    expect(windows?.presentCount).toBe(2);
    expect(catalog.required("group-latin-windows-1252")).toContain(0x20ac);
    expect(catalog.required("group-latin-windows-1252")).not.toContain(0x81);
  });

  it("covers the scripts of the Glyphs sidebar that Hyperglot lacks", () => {
    const catalog = db.createLanguageCatalog([]);
    const scripts = catalog.scripts.map(({ script }) => script);

    for (const script of ["Adlam", "Brahmi", "Braille", "Kawi", "Ogham", "Runes"]) {
      expect(scripts).toContain(script);
    }
    expect(scripts).not.toContain("Yezidi");
  });

  it("filters the font repertoire by a group", () => {
    const catalog = db.createLanguageCatalog([0x42, 0x24, 0x41]);

    expect(catalog.filter("group-latin-basic")).toEqual([0x42, 0x41]);
    expect(catalog.groupCoverage("unknown-group")).toBeNull();
  });

  it("keeps group identifiers unique and apart from language identifiers", () => {
    const catalog = db.createLanguageCatalog([]);
    const identifiers = catalog.scripts.flatMap((script) => [
      ...script.groups.map(({ group }) => group.id),
      ...script.languages.map(({ language }) => language.id),
    ]);

    expect(new Set(identifiers).size).toBe(identifiers.length);
  });

  it("tracks every group of Latin, Cyrillic and Greek and the languages that need more than basic Latin", () => {
    const catalog = db.createLanguageCatalog([]);
    const tracked = db.defaultTrackedLanguageIds();
    const scripts = catalog.scriptsFor(tracked);
    const latin = scripts.find(({ script }) => script === "Latin");
    const basic = new Set(catalog.groupCoverage("group-latin-basic")?.group.baseCodepoints);

    expect(DEFAULT_SCRIPTS).toEqual(["Latin", "Cyrillic", "Greek"]);
    expect(scripts.map(({ script }) => script)).toEqual([
      "Constructed languages",
      "Cyrillic",
      "Greek",
      "Latin",
    ]);
    for (const script of scripts) {
      const all = catalog.scripts.find((entry) => entry.script === script.script);
      expect(script.groups.map(({ group }) => group.id)).toEqual(
        all?.groups.map(({ group }) => group.id),
      );
    }

    const latinIds = new Set(latin?.languages.map(({ language }) => language.id));
    for (const id of ["fra-latin", "hau-latin", "ita-latin", "por-latin", "spa-latin"]) {
      expect(latinIds.has(id)).toBe(true);
    }
    expect(latinIds.has("eng-latin")).toBe(false);
    expect(latinIds.has("xho-latin")).toBe(false);
    for (const { language } of latin?.languages ?? []) {
      const extra = language.baseCodepoints.filter(
        (codepoint) => !basic.has(codepoint) && ![0xc6, 0xe6, 0x152, 0x153].includes(codepoint),
      );
      expect(extra.length).toBeGreaterThan(0);
    }

    const greek = scripts.find(({ script }) => script === "Greek");
    expect(greek?.languages.some(({ language }) => language.id === "ell-greek")).toBe(true);
  });

  it("tracks every Cyrillic language, including the non-Slavic ones", () => {
    const tracked = new Set(db.defaultTrackedLanguageIds());
    const catalog = db.createLanguageCatalog([]);
    const cyrillic = catalog.scripts.find(({ script }) => script === "Cyrillic");

    expect(cyrillic?.languages.length).toBeGreaterThan(80);
    for (const { language } of cyrillic?.languages ?? []) {
      if (language.orthography === "primary") expect(tracked.has(language.id)).toBe(true);
    }
    for (const id of [
      "abk-cyrillic",
      "che-cyrillic",
      "kaz-cyrillic",
      "oss-cyrillic",
      "tgk-cyrillic",
    ]) {
      expect(tracked.has(id)).toBe(true);
    }
  });

  it("lists constructed languages and Esperanto under their own category", () => {
    const catalog = db.createLanguageCatalog([]);
    const constructed = catalog.scripts.find(({ script }) => script === "Constructed languages");
    const names = constructed?.languages.map(({ language }) => language.name) ?? [];

    expect(names).toContain("Esperanto");
    expect(names).toContain("Klingon");
    expect(names).not.toContain("Standard Moroccan Tamazight");
    expect(constructed?.groups.map(({ group }) => group.name)).toEqual(["Esperanto"]);
    expect(
      catalog.scripts
        .find(({ script }) => script === "Latin")
        ?.languages.some(({ language }) => language.id === "epo-latin"),
    ).toBe(false);
  });

  it("names the orthographies of one language apart", () => {
    const catalog = db.createLanguageCatalog([]);
    for (const script of catalog.scripts) {
      const names = script.languages.map(({ language }) => language.name);
      expect(new Set(names).size).toBe(names.length);
    }
    const georgian = catalog.scripts.find(({ script }) => script === "Georgian");
    expect(georgian?.languages.map(({ language }) => language.name)).toEqual(
      expect.arrayContaining([
        "Georgian (Asomtavruli)",
        "Georgian (Mkhedruli and Mtavruli)",
        "Georgian (Nuskhuri)",
      ]),
    );
  });

  it("lists the romanisation groups under Latin and the merged Math group", () => {
    const catalog = db.createLanguageCatalog([]);
    const latin = catalog.scripts.find(({ script }) => script === "Latin");
    const romanization = latin?.groups
      .filter(({ group }) => group.section === "Romanization")
      .map(({ group }) => group.name);
    expect(romanization).toEqual([
      "Pinyin",
      "Arabic ALA-LC",
      "Arabic DMG",
      "Arabic ISO 233",
      "Sanskrit",
    ]);

    const math = catalog.scripts.find(({ script }) => script === "Math");
    expect(math?.groups.map(({ group }) => group.name)).toEqual([
      "Letters",
      "Figures",
      "Punctuation and Symbols",
    ]);
  });

  it("recognises European languages by their ISO code", () => {
    expect(isEuropeanLanguageId("fra-latin")).toBe(true);
    expect(isEuropeanLanguageId("rus-cyrillic")).toBe(true);
    expect(isEuropeanLanguageId("hau-latin")).toBe(false);
  });

  it("lists custom groups after the built-in groups of their script", () => {
    const custom: LanguageGroup = {
      id: "custom-klingon-letters",
      name: "Letters",
      script: "Klingon",
      section: null,
      baseCodepoints: [0xf8d0, 0xf8d1],
      basis: "custom",
    };
    const catalog = db.createLanguageCatalog([0xf8d0], [custom]);
    const klingon = catalog.scripts.find(({ script }) => script === "Klingon");

    expect(klingon?.groups[0]?.presentCount).toBe(1);
    expect(klingon?.groups[0]?.requiredCount).toBe(2);
    expect(catalog.filter("custom-klingon-letters")).toEqual([0xf8d0]);
    expect(catalog.scriptsFor(["custom-klingon-letters"]).map(({ script }) => script)).toEqual([
      "Klingon",
    ]);
  });

  it("lets a custom group replace a built-in group with the same identifier", () => {
    const replacement: LanguageGroup = {
      id: "group-latin-basic",
      name: "Basic",
      script: "Latin",
      section: null,
      baseCodepoints: [0x41],
      basis: "custom",
    };
    const catalog = db.createLanguageCatalog([0x41], [replacement]);

    expect(catalog.required("group-latin-basic")).toEqual([0x41]);
    expect(catalog.groupCoverage("group-latin-basic")?.group.basis).toBe("custom");
  });

  it("finds the codepoint of a glyph name, production name, or alias", () => {
    expect(db.codepointForGlyphName("aacute")).toBe(0xe1);
    expect(db.codepointForGlyphName("nonexistentGlyph")).toBeNull();
  });

  it("keeps derived groups inside the codepoints Glyphs generates", () => {
    const catalog = db.createLanguageCatalog([]);
    const yezidi = catalog.scripts.find(({ script }) => script === "Yezidi");

    expect(yezidi?.groups ?? []).toEqual([]);
    expect(catalog.required("group-latin-ipa").length).toBeLessThan(150);
  });

  it("sorts scripts and languages alphabetically", () => {
    const catalog = db.createLanguageCatalog([]);
    const scripts = catalog.scripts.map(({ script }) => script);
    const latin = catalog.scripts.find((script) => script.script === "Latin");
    const languageNames = latin?.languages.map(({ language }) => language.name) ?? [];

    expect(scripts).toEqual([...scripts].sort((left, right) => left.localeCompare(right)));
    expect(languageNames).toEqual(
      [...languageNames].sort((left, right) => left.localeCompare(right)),
    );
  });
});

describe("search", () => {
  it("finds dollar sign by name", () => {
    const results = db.search("dollar");
    expect(results.length).toBeGreaterThan(0);
    expect(results.some((r) => r.codepoint === 0x24)).toBe(true);
  });

  it("finds arrow glyphs", () => {
    const results = db.search("arrow");
    expect(results.length).toBeGreaterThan(0);
  });

  it("supports prefix matching", () => {
    const results = db.search("dol");
    expect(results.length).toBeGreaterThan(0);
    expect(results.some((r) => r.codepoint === 0x24)).toBe(true);
  });

  it("returns empty for empty query", () => {
    expect(db.search("")).toEqual([]);
  });

  it("returns empty for whitespace-only query", () => {
    expect(db.search("   ")).toEqual([]);
    expect(db.search("\t\n")).toEqual([]);
  });

  it("respects limit parameter", () => {
    const results = db.search("latin", 5);
    expect(results.length).toBeLessThanOrEqual(5);
  });

  it("default limit is 50", () => {
    const results = db.search("latin");
    expect(results.length).toBeLessThanOrEqual(50);
  });

  it("results have all required fields", () => {
    const results = db.search("dollar");
    for (const r of results) {
      expect(r.codepoint).toBeTypeOf("number");
      expect(r.rank).toBeTypeOf("number");
      // glyphName, unicodeName, category, subCategory can be null
    }
  });

  it("finds by unicode name", () => {
    const results = db.search("EXCLAMATION MARK");
    expect(results.length).toBeGreaterThan(0);
    expect(results.some((r) => r.codepoint === 0x21)).toBe(true);
  });

  it("finds by glyph name", () => {
    const results = db.search("exclam");
    expect(results.length).toBeGreaterThan(0);
    expect(results.some((r) => r.codepoint === 0x21)).toBe(true);
  });

  it("multi-word search narrows results", () => {
    const broad = db.search("latin");
    const narrow = db.search("latin capital");
    expect(narrow.length).toBeLessThanOrEqual(broad.length);
  });

  it("search with limit 1 returns at most 1 result", () => {
    const results = db.search("dollar", 1);
    expect(results.length).toBeLessThanOrEqual(1);
  });

  it("results are ranked (rank values are present)", () => {
    const results = db.search("dollar");
    expect(results.length).toBeGreaterThan(0);
    for (const r of results) {
      expect(r.rank).toBeTypeOf("number");
    }
  });

  it("finds characters by category search", () => {
    const results = db.search("Currency");
    expect(results.length).toBeGreaterThan(0);
  });

  it("handles special characters gracefully", () => {
    // These should not crash — they may return no results
    expect(() => db.search('"')).not.toThrow();
    expect(() => db.search("(")).not.toThrow();
    expect(() => db.search(")")).not.toThrow();
  });
});

describe("category helpers", () => {
  it("resolves known codepoint category metadata", () => {
    const result = db.getCategoryForCodepoint(0x24);
    expect(result).toEqual({
      category: "Symbol",
      subCategoryKey: "Currency",
      subCategoryLabel: "Currency",
      isKnown: true,
    });
  });

  it("resolves unknown codepoint using defaults", () => {
    const result = db.getCategoryForCodepoint(0xfffe);
    expect(result).toEqual({
      category: "Other",
      subCategoryKey: "__null_subcategory__",
      subCategoryLabel: "General",
      isKnown: false,
    });
  });

  it("supports overriding fallback labels", () => {
    const result = db.getCategoryForCodepoint(0xfffe, {
      unknownCategoryLabel: "Unknown",
      nullSubCategoryKey: "none",
      nullSubCategoryLabel: "None",
    });
    expect(result).toEqual({
      category: "Unknown",
      subCategoryKey: "none",
      subCategoryLabel: "None",
      isKnown: false,
    });
  });

  it("builds sorted category summaries with subcategory counts", () => {
    const summaries = db.getCategorySummaries([0x24, 0x20, 0x30, 0xfffe]);
    expect(summaries.map((summary) => summary.category)).toEqual([
      "Number",
      "Other",
      "Separator",
      "Symbol",
    ]);

    const symbolSummary = summaries.find((summary) => summary.category === "Symbol");
    expect(symbolSummary).toBeDefined();
    expect(symbolSummary!.count).toBe(1);
    expect(symbolSummary!.subCategories).toEqual([
      { key: "Currency", label: "Currency", count: 1 },
    ]);
  });

  it("can exclude unknown codepoints from summaries", () => {
    const summaries = db.getCategorySummaries([0x24, 0xfffe], { includeUnknown: false });
    expect(summaries.map((summary) => summary.category)).toEqual(["Symbol"]);
  });

  it("filters by category and subcategory", () => {
    const codepoints = [0x24, 0xa3, 0x20, 0x30, 0xfffe];
    const categoryOnly = db.filterCodepoints(codepoints, { category: "Symbol" });
    expect(categoryOnly).toEqual([0x24, 0xa3]);

    const subCategory = db.filterCodepoints(codepoints, {
      category: "Symbol",
      subCategoryKey: "Currency",
    });
    expect(subCategory).toEqual([0x24, 0xa3]);
  });

  it("filters by full-text query while preserving input order", () => {
    const codepoints = [0x41, 0x42, 0x24, 0x21];
    const filtered = db.filterCodepoints(codepoints, { query: "exclam" });
    expect(filtered).toEqual([0x21]);
  });

  it("returns empty list for subcategory without category", () => {
    const codepoints = [0x24, 0x23, 0x20];
    const filtered = db.filterCodepoints(codepoints, { subCategoryKey: "Currency" });
    expect(filtered).toEqual([]);
  });

  it("can exclude unknown codepoints from filtering", () => {
    const codepoints = [0x24, 0xfffe];
    const filtered = db.filterCodepoints(codepoints, {}, { includeUnknown: false });
    expect(filtered).toEqual([0x24]);
  });

  it("creates reusable catalogs with summaries and filtering", () => {
    const catalog = db.createCategoryCatalog([0x24, 0xa3, 0x30, 0xfffe]);
    expect(catalog.categories.map((summary) => summary.category)).toEqual([
      "Number",
      "Other",
      "Symbol",
    ]);

    const symbolOnly = catalog.filter({ category: "Symbol" });
    expect(symbolOnly).toEqual([0x24, 0xa3]);
  });
});

describe("cross-domain consistency", () => {
  it("charset codepoints that are in glyphData return valid glyph names", () => {
    const codepoints = db.getCharsetCodepoints("adobe-latin-1");
    let matchCount = 0;
    for (const cp of codepoints) {
      const name = db.getGlyphName(cp);
      if (name !== null) {
        matchCount++;
        expect(name.length).toBeGreaterThan(0);
      }
    }
    // Most Adobe Latin 1 codepoints should be in Glyph.xml
    expect(matchCount).toBeGreaterThan(200);
  });

  it("decomposable characters in the charset can be looked up", () => {
    const codepoints = db.getCharsetCodepoints("adobe-latin-1");
    let decomposableCount = 0;
    for (const cp of codepoints) {
      const decomp = db.getDecomposition(cp);
      if (decomp.length > 0) {
        decomposableCount++;
        // Each component should be a valid codepoint
        for (const comp of decomp) {
          expect(comp).toBeGreaterThanOrEqual(0);
          expect(comp).toBeLessThanOrEqual(0xffff);
        }
      }
    }
    // Accented chars like é, ñ, ü etc. should decompose
    expect(decomposableCount).toBeGreaterThan(10);
  });

  it("searchable codepoints match glyph data lookups", () => {
    const searchResults = db.search("dollar");
    const dollarResult = searchResults.find((r) => r.codepoint === 0x24);
    expect(dollarResult).toBeDefined();

    const glyphData = db.getGlyph(0x24);
    expect(glyphData).not.toBeNull();
    expect(dollarResult!.glyphName).toBe(glyphData!.name);
  });
});

describe("component resources loaded later", () => {
  it("answers names at once and decomposition and search once added", () => {
    const info = new GlyphInfo(coreResources);
    expect(info.getGlyphName(0xe9)).toBe("eacute");
    expect(info.hasComponentResources).toBe(false);
    expect(info.getDecomposition(0xe9)).toEqual([]);
    expect(info.search("eacute")).toEqual([]);

    info.addResources(componentResources);
    expect(info.hasComponentResources).toBe(true);
    expect(info.getDecomposition(0xe9)).toEqual(db.getDecomposition(0xe9));
    expect(info.search("eacute")).toEqual(db.search("eacute"));
  });
});
