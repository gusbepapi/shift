import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { groupsFromGlyphsDump, type GlyphsSidebarDump } from "../../src/glyphsSidebar.js";
import type { Glyph, GlyphCategory, Language, LanguageGroup } from "../../src/types.js";

/**
 * Builds the Glyphs-style groups shown under each script in the sidebar.
 *
 * The membership lists of Glyphs' own groups are not published, so most groups
 * here are derived: from Hyperglot orthographies for the language groups, and
 * from the per-script categories of GlyphData.xml for the other scripts. Only
 * the legacy encodings are exact. Every group records which of the two it is.
 */

type Basis = LanguageGroup["basis"];

const DATA_DIRECTORY = join(dirname(fileURLToPath(import.meta.url)), "..", "data");
const GLYPHS_UNION_PATH = join(DATA_DIRECTORY, "glyphs-union.json");

/**
 * The Languages sidebar as `DumpLanguagesSidebar.py` writes it. When the file exists, it replaces the
 * derived groups with Glyphs' own, which are exact.
 */
const GLYPHS_SIDEBAR_PATH = join(DATA_DIRECTORY, "glyphs-sidebar.json");

/** Sidebar rows that came from a user's own `Groups.plist` rather than from Glyphs. */
const USER_DEFINED_ROWS: ReadonlySet<string> = new Set([
  "Akkadian (Sumero-Akkadian cuneiform)",
  "Klingon (pIqaD)",
]);

/**
 * Scripts that Glyphs lists as one row with no subgroups; every category of the
 * script belongs to that row.
 */
const FLAT_SCRIPTS: ReadonlySet<string> = new Set(["Armenian", "Elymaic", "Syriac"]);

/**
 * Reads the codepoints that Glyphs generated into a font holding every group.
 *
 * @remarks
 * No group of Glyphs can hold a codepoint outside this set, so it bounds the
 * derived groups from above.
 */
function readGlyphsUnion(): ReadonlySet<number> {
  const { ranges } = JSON.parse(readFileSync(GLYPHS_UNION_PATH, "utf-8")) as {
    ranges: [number, number][];
  };
  const union = new Set<number>();
  for (const [from, to] of ranges) {
    for (let codepoint = from; codepoint <= to; codepoint += 1) union.add(codepoint);
  }
  return union;
}

interface GroupDraft {
  name: string;
  section?: string;
  basis: Basis;
  codepoints: Iterable<number>;
}

interface CategoryScript {
  /** Script name as Hyperglot spells it, so the groups join the existing script. */
  script: string;
  /** `script` values of GlyphData.xml that belong to it. */
  glyphDataScripts: readonly string[];
}

const CATEGORY_GROUP_NAMES: Partial<Record<GlyphCategory, string>> = {
  Letter: "Letters",
  Number: "Figures",
  Punctuation: "Punctuation",
  Symbol: "Symbols",
  Mark: "Marks",
};

/** Scripts of the Glyphs sidebar that are grouped by category rather than by language. */
const CATEGORY_SCRIPTS: readonly CategoryScript[] = [
  { script: "Adlam", glyphDataScripts: ["adlam"] },
  { script: "Armenian", glyphDataScripts: ["armenian"] },
  { script: "Balinese", glyphDataScripts: ["balinese"] },
  { script: "Batak", glyphDataScripts: ["batak"] },
  { script: "Bengali", glyphDataScripts: ["bengali"] },
  { script: "Brahmi", glyphDataScripts: ["brahmi"] },
  { script: "Braille", glyphDataScripts: ["braille"] },
  { script: "Buginese", glyphDataScripts: ["buginese"] },
  { script: "Burmese", glyphDataScripts: ["myanmar"] },
  { script: "Cham", glyphDataScripts: ["cham"] },
  { script: "Devanagari", glyphDataScripts: ["devanagari"] },
  { script: "Elymaic", glyphDataScripts: ["elymaic"] },
  { script: "Geʽez", glyphDataScripts: ["ethiopic"] },
  { script: "Gujarati", glyphDataScripts: ["gujarati"] },
  { script: "Gurmukhi", glyphDataScripts: ["gurmukhi"] },
  { script: "Javanese", glyphDataScripts: ["javanese"] },
  { script: "Kannada", glyphDataScripts: ["kannada"] },
  { script: "Kawi", glyphDataScripts: ["kawi"] },
  { script: "Khmer", glyphDataScripts: ["khmer"] },
  { script: "Lao", glyphDataScripts: ["lao"] },
  { script: "Malayalam", glyphDataScripts: ["malayalam"] },
  { script: "New Tai Lue", glyphDataScripts: ["lue"] },
  { script: "N'Ko", glyphDataScripts: ["nko"] },
  { script: "Ogham", glyphDataScripts: ["ogham"] },
  { script: "Old Hungarian", glyphDataScripts: ["rovas"] },
  { script: "Oriya", glyphDataScripts: ["oriya"] },
  { script: "Osmanya", glyphDataScripts: ["osmanya"] },
  { script: "Runes", glyphDataScripts: ["runic"] },
  { script: "Sinhala", glyphDataScripts: ["sinhala"] },
  { script: "Syriac", glyphDataScripts: ["syriac"] },
  { script: "Tagalog", glyphDataScripts: ["tagalog"] },
  { script: "Tamil", glyphDataScripts: ["tamil"] },
  { script: "Thai", glyphDataScripts: ["thai"] },
  { script: "Tham", glyphDataScripts: ["tham"] },
  { script: "Tibetan", glyphDataScripts: ["tibet"] },
  { script: "Tifinagh", glyphDataScripts: ["tifinagh"] },
  { script: "Yezidi", glyphDataScripts: ["yezidi"] },
];

/**
 * Latin regional groups, as ISO 639-3 codes of Hyperglot languages whose Latin
 * orthography feeds the group. The regions follow the names in Glyphs, whose own
 * boundaries are soft, so membership here is an approximation.
 */
const LATIN_REGIONS: Readonly<Record<string, readonly string[]>> = {
  "Western European": [
    "eng",
    "fra",
    "deu",
    "nld",
    "ita",
    "spa",
    "por",
    "cat",
    "glg",
    "oci",
    "ast",
    "arg",
    "bre",
    "cym",
    "gle",
    "gla",
    "glv",
    "cor",
    "ltz",
    "fry",
    "nds",
    "dan",
    "nor",
    "swe",
    "isl",
    "fao",
    "fin",
    "kal",
    "eus",
    "roh",
    "fur",
    "lld",
    "gsw",
    "lad",
    "lij",
    "lmo",
    "nap",
    "pms",
    "scn",
    "srd",
    "vec",
    "wln",
    "pcd",
    "sco",
    "mwl",
    "wae",
    "ksh",
    "cim",
  ],
  "Central European": [
    "pol",
    "ces",
    "slk",
    "hun",
    "hrv",
    "bos",
    "slv",
    "hsb",
    "dsb",
    "csb",
    "szl",
    "lit",
    "lvs",
    "ltg",
    "ekk",
    "prg",
    "deu",
  ],
  "South Eastern European": [
    "ron",
    "ruo",
    "rup",
    "ruq",
    "als",
    "aln",
    "aae",
    "aat",
    "tur",
    "azj",
    "crh",
    "gag",
    "mlt",
    "cnr",
    "srp",
    "bos",
    "hrv",
    "rmn",
    "rmy",
  ],
  "South American": [
    "arn",
    "ayc",
    "ayr",
    "agr",
    "acu",
    "amc",
    "amr",
    "apn",
    "arl",
    "arr",
    "awe",
    "cbr",
    "cbs",
    "cbt",
    "cbu",
    "cof",
    "con_",
    "cni",
    "cot",
    "cpu",
    "ese",
    "gug",
    "guc",
    "huu",
    "kgp",
    "kwi",
    "mcd",
    "mcf",
    "miq",
    "not",
    "pbb",
    "prq",
    "que",
    "sey",
    "shp",
    "jiv",
    "snn",
    "tca",
    "tob",
    "ura",
    "yad",
    "zro",
    "auc",
    "guu",
    "xav",
    "srn",
    "srm",
  ],
  Oceanian: [
    "mri",
    "rar",
    "haw",
    "smo",
    "ton",
    "tvl",
    "tkl",
    "niu",
    "tah",
    "mrq",
    "mqm",
    "fij",
    "gil",
    "mah",
    "pon",
    "chk",
    "pau",
    "yap",
    "cha",
    "wls",
    "ulk",
    "aud",
    "aer",
    "wbp",
    "mwf",
    "pis",
    "tpi",
    "bis",
    "roo",
    "mwp",
    "wyb",
    "wrh",
    "yij",
    "piu",
    "gni",
    "wim",
    "dhv",
    "plz",
  ],
  "Pan African": [
    "aar",
    "abi",
    "abr",
    "abn",
    "ach",
    "ada",
    "ade",
    "afr",
    "agq",
    "ajg",
    "aha",
    "any",
    "anw",
    "anc",
    "asa",
    "avn",
    "ayb",
    "azo",
    "bam",
    "bas",
    "bba",
    "bbo",
    "bci",
    "bem",
    "bez",
    "bfa",
    "bfo",
    "bin",
    "bib",
    "bjt",
    "bkm",
    "bmq",
    "boz",
    "box",
    "bqc",
    "bsc",
    "bss",
    "bwj",
    "bwq",
    "byv",
    "bze",
    "cgg",
    "cjk",
    "cko",
    "cme",
    "crs",
    "dag",
    "dav",
    "ddn",
    "dga",
    "dgi",
    "dgl",
    "din",
    "dip",
    "dje",
    "dop",
    "dua",
    "dya",
    "dyo",
    "dyu",
    "ebu",
    "efi",
    "emk",
    "ewe",
    "ewo",
    "fat",
    "fia",
    "fmp",
    "fon",
    "fod",
    "fub",
    "fue",
    "fuf",
    "fuh",
    "fui",
    "fuq",
    "fuv",
    "fuc",
    "ffm",
    "gaa",
    "gaz",
    "gax",
    "gjn",
    "gkp",
    "guz",
    "gur",
    "gux",
    "hae",
    "hau",
    "ibb",
    "ibo",
    "idu",
    "ife",
    "jgo",
    "kab",
    "kam",
    "kau",
    "kbp",
    "kdh",
    "kde",
    "kea",
    "khq",
    "kik",
    "kin",
    "kkj",
    "kln",
    "kmb",
    "knf",
    "kon",
    "koo",
    "kqn",
    "kqs",
    "kri",
    "ksf",
    "kst",
    "ktu",
    "kua",
    "kus",
    "lag",
    "lam",
    "lee",
    "lia",
    "lin",
    "lns",
    "lob",
    "loz",
    "lua",
    "lub",
    "lug",
    "luo",
    "mas",
    "men",
    "mer",
    "mey",
    "mfe",
    "mfq",
    "mfv",
    "mgh",
    "mgo",
    "mlg",
    "mnk",
    "mos",
    "mql",
    "mua",
    "naq",
    "nbl",
    "nde",
    "ndo",
    "nku",
    "nmg",
    "nmz",
    "nnh",
    "nnw",
    "nso",
    "nus",
    "nya",
    "nym",
    "nyn",
    "nza",
    "nzi",
    "orc",
    "pov",
    "run",
    "rwk",
    "sag",
    "saq",
    "sav",
    "sbd",
    "sbp",
    "seh",
    "ses",
    "shi",
    "sld",
    "snf",
    "snk",
    "sna",
    "som",
    "sot",
    "srr",
    "ssw",
    "suk",
    "swb",
    "swc",
    "swh",
    "sxw",
    "tbz",
    "tem",
    "teo",
    "tik",
    "tiv",
    "tnr",
    "tod",
    "toi",
    "tso",
    "tsn",
    "tum",
    "twi",
    "twq",
    "ttq",
    "tzm",
    "ven",
    "vmw",
    "wci",
    "wmw",
    "wol",
    "wss",
    "wwa",
    "xho",
    "xnz",
    "xog",
    "xsm",
    "xwe",
    "yao",
    "yav",
    "ymk",
    "yor",
    "zdj",
    "zgh",
    "zul",
  ],
};

const BASIC_LATIN_LETTERS =
  "A B C D E F G H I J K L M N O P Q R S T U V W X Y Z a b c d e f g h i j k l m n o p q r s t u v w x y z";

/** Splits a space-separated list of characters into codepoints. */
function codepointsOf(characters: string): number[] {
  return characters
    .split(" ")
    .filter((character) => character !== "")
    .map((character) => character.codePointAt(0) as number);
}

/**
 * The characters each romanisation scheme adds to the basic Latin letters, as the schemes publish
 * them; Glyphs' own membership may differ until its sidebar is dumped.
 */
const ROMANIZATIONS: readonly { name: string; characters: string }[] = [
  {
    name: "Pinyin",
    characters:
      "ā á ǎ à ē é ě è ī í ǐ ì ō ó ǒ ò ū ú ǔ ù ǖ ǘ ǚ ǜ ü Ā Á Ǎ À Ē É Ě È Ī Í Ǐ Ì Ō Ó Ǒ Ò Ū Ú Ǔ Ù Ǖ Ǘ Ǚ Ǜ Ü ê Ê ế Ế ề Ề ń ň ǹ Ń Ň Ǹ ḿ Ḿ ɑ ɡ ĉ ŝ ẑ Ĉ Ŝ Ẑ \u0300 \u0301 \u0302 \u0304 \u0308 \u030c",
  },
  {
    name: "Arabic ALA-LC",
    characters: "ā ī ū Ā Ī Ū ḍ ḥ ṣ ṭ ẓ Ḍ Ḥ Ṣ Ṭ Ẓ ʻ ʼ ʾ ʿ \u0304 \u0361",
  },
  {
    name: "Arabic DMG",
    characters: "ā ī ū Ā Ī Ū ḏ ṯ ḥ ḫ š ṣ ḍ ṭ ẓ ġ ǧ Ḏ Ṯ Ḥ Ḫ Š Ṣ Ḍ Ṭ Ẓ Ġ Ǧ ʾ ʿ",
  },
  {
    name: "Arabic ISO 233",
    characters: "ā ī ū ã Ā Ī Ū ḏ ṯ ḥ ḫ š ṣ ḍ ṭ ẓ ġ ǧ Ḏ Ṯ Ḥ Ḫ Š Ṣ Ḍ Ṭ Ẓ Ġ Ǧ ẗ ỳ ʾ ʿ",
  },
  {
    name: "Sanskrit",
    characters: "ā ī ū ṛ ṝ ḷ ḹ ṃ ṁ ḥ ṅ ñ ṭ ḍ ṇ ś ṣ ē ō ṟ ḻ ṉ Ā Ī Ū Ṛ Ṝ Ḷ Ḹ Ṃ Ṁ Ḥ Ṅ Ñ Ṭ Ḍ Ṇ Ś Ṣ",
  },
];

const SAMI_CODES = ["sma", "sme", "smj", "smn", "sms", "sju", "sje"] as const;

export function generateGroups(
  languagesPath: string,
  glyphDataPath: string,
  outputPath: string,
): number {
  const languages = (JSON.parse(readFileSync(languagesPath, "utf-8")) as { languages: Language[] })
    .languages;
  const glyphs = JSON.parse(readFileSync(glyphDataPath, "utf-8")) as Glyph[];

  if (existsSync(GLYPHS_SIDEBAR_PATH)) {
    return writeSidebarGroups(glyphs, outputPath);
  }

  const glyphsUnion = readGlyphsUnion();
  const groups: LanguageGroup[] = [];
  const addScript = (script: string, drafts: readonly GroupDraft[]) => {
    for (const draft of drafts) {
      const bounded =
        draft.basis === "derived"
          ? [...draft.codepoints].filter((codepoint) => glyphsUnion.has(codepoint))
          : [...draft.codepoints];
      const baseCodepoints = [...new Set(bounded)].sort((left, right) => left - right);
      if (baseCodepoints.length === 0) continue;

      groups.push({
        id: `group-${slugify(script)}-${slugify(draft.name)}`,
        name: draft.name,
        script,
        section: draft.section ?? null,
        baseCodepoints,
        basis: draft.basis,
      });
    }
  };

  const languageCodepoints = (script: string, codes: readonly string[]): number[] => {
    const wanted = new Set(codes);
    return languages
      .filter(
        (language) =>
          language.script === script &&
          language.orthography === "primary" &&
          wanted.has(language.id.slice(0, language.id.indexOf(`-${slugify(script)}`))),
      )
      .flatMap((language) => language.baseCodepoints);
  };
  const glyphCodepoints = (predicate: (glyph: Glyph) => boolean): number[] =>
    glyphs.filter(predicate).map((glyph) => glyph.codepoint);
  const allOfScript = (script: string): number[] =>
    languages
      .filter((language) => language.script === script)
      .flatMap((language) => language.baseCodepoints);

  addScript("Latin", [
    { name: "Basic", basis: "derived", codepoints: [...range(0x41, 0x5a), ...range(0x61, 0x7a)] },
    ...Object.entries(LATIN_REGIONS).map(
      ([name, codes]): GroupDraft => ({
        name,
        basis: "derived",
        codepoints: languageCodepoints("Latin", codes),
      }),
    ),
    { name: "Vietnamese", basis: "derived", codepoints: languageCodepoints("Latin", ["vie"]) },
    { name: "Sámi", basis: "derived", codepoints: languageCodepoints("Latin", SAMI_CODES) },
    {
      name: "IPA",
      basis: "derived",
      codepoints: glyphCodepoints(
        (glyph) =>
          glyph.script === "latin" &&
          ((glyph.codepoint >= 0x250 && glyph.codepoint <= 0x2ff) ||
            (glyph.codepoint >= 0x1d00 && glyph.codepoint <= 0x1dbf)),
      ),
    },
    { name: "Esperanto", basis: "derived", codepoints: languageCodepoints("Latin", ["epo"]) },
    {
      name: "Windows 1252",
      section: "Character Sets",
      basis: "exact",
      codepoints: decodeEncoding("windows-1252"),
    },
    {
      name: "Mac Roman",
      section: "Character Sets",
      basis: "exact",
      codepoints: decodeEncoding("macintosh"),
    },
    ...ROMANIZATIONS.map(({ name, characters }) => ({
      name,
      section: "Romanization",
      basis: "derived" as const,
      codepoints: [...codepointsOf(BASIC_LATIN_LETTERS), ...codepointsOf(characters)],
    })),
  ]);

  addScript("Cyrillic", [
    { name: "Basic", basis: "derived", codepoints: languageCodepoints("Cyrillic", ["rus"]) },
    { name: "Belarusian", basis: "derived", codepoints: languageCodepoints("Cyrillic", ["bel"]) },
    { name: "Bulgarian", basis: "derived", codepoints: languageCodepoints("Cyrillic", ["bul"]) },
    { name: "Russian", basis: "derived", codepoints: languageCodepoints("Cyrillic", ["rus"]) },
    {
      name: "Serbian and Macedonian",
      basis: "derived",
      codepoints: languageCodepoints("Cyrillic", ["srp", "mkd"]),
    },
    { name: "Ukrainian", basis: "derived", codepoints: languageCodepoints("Cyrillic", ["ukr"]) },
    { name: "Extended", basis: "derived", codepoints: allOfScript("Cyrillic") },
  ]);

  addScript("Greek", [
    { name: "Basic", basis: "derived", codepoints: languageCodepoints("Greek", ["ell"]) },
    {
      name: "Polytonic",
      basis: "derived",
      codepoints: glyphCodepoints(
        (glyph) =>
          glyph.script === "greek" && glyph.codepoint >= 0x1f00 && glyph.codepoint <= 0x1fff,
      ),
    },
    {
      name: "Coptic",
      basis: "derived",
      codepoints: glyphCodepoints((glyph) => glyph.script === "coptic"),
    },
  ]);

  addScript("Arabic", [
    { name: "Basic", basis: "derived", codepoints: languageCodepoints("Arabic", ["arb"]) },
    { name: "Persian", basis: "derived", codepoints: languageCodepoints("Arabic", ["pes", "fas"]) },
    { name: "Urdu", basis: "derived", codepoints: languageCodepoints("Arabic", ["urd"]) },
    { name: "Kurdish", basis: "derived", codepoints: languageCodepoints("Arabic", ["ckb", "kmr"]) },
  ]);

  addScript("Hebrew", [
    { name: "Basic", basis: "derived", codepoints: languageCodepoints("Hebrew", ["heb"]) },
    { name: "Yiddish", basis: "derived", codepoints: languageCodepoints("Hebrew", ["ydd"]) },
    {
      name: "Punctuation",
      basis: "derived",
      codepoints: glyphCodepoints(
        (glyph) => glyph.script === "hebrew" && glyph.category === "Punctuation",
      ),
    },
    {
      name: "Diacritic Marks",
      basis: "derived",
      codepoints: glyphCodepoints(
        (glyph) => glyph.script === "hebrew" && glyph.category === "Mark",
      ),
    },
  ]);

  addScript("Georgian", [
    ...categoryDrafts(glyphs, ["georgian"]),
    {
      name: "Old Georgian",
      basis: "derived",
      codepoints: glyphCodepoints((glyph) => glyph.script === "old georgian"),
    },
  ]);

  addScript("Japanese", [
    {
      name: "Kana",
      basis: "derived",
      codepoints: glyphCodepoints((glyph) => glyph.script === "kana"),
    },
  ]);

  addScript("North American", [
    {
      name: "Cherokee",
      basis: "derived",
      codepoints: glyphCodepoints((glyph) => glyph.script === "cherokee"),
    },
    {
      name: "Canadian",
      basis: "derived",
      codepoints: glyphCodepoints((glyph) => glyph.script === "canadian"),
    },
  ]);

  addScript("Music", [
    {
      name: "All",
      basis: "derived",
      codepoints: glyphCodepoints((glyph) => glyph.script === "musical"),
    },
  ]);

  addScript("Currency", [
    {
      name: "Currency",
      basis: "derived",
      codepoints: glyphCodepoints((glyph) => glyph.subCategory === "Currency"),
    },
  ]);

  const mathGlyphs = glyphs.filter((glyph) => glyph.script === "math");
  addScript("Math", [
    {
      name: "Letters",
      basis: "derived",
      codepoints: mathGlyphs
        .filter((glyph) => glyph.category === "Letter")
        .map((glyph) => glyph.codepoint),
    },
    {
      name: "Figures",
      basis: "derived",
      codepoints: mathGlyphs
        .filter((glyph) => glyph.category === "Number")
        .map((glyph) => glyph.codepoint),
    },
    {
      name: "Punctuation and Symbols",
      basis: "derived",
      codepoints: mathGlyphs
        .filter((glyph) => ["Punctuation", "Symbol", "Separator"].includes(glyph.category))
        .map((glyph) => glyph.codepoint),
    },
  ]);

  for (const { script, glyphDataScripts } of CATEGORY_SCRIPTS) {
    const drafts = categoryDrafts(glyphs, glyphDataScripts);
    addScript(
      script,
      FLAT_SCRIPTS.has(script)
        ? [
            {
              name: script,
              basis: "derived",
              codepoints: drafts.flatMap((draft) => [...draft.codepoints]),
            },
          ]
        : drafts,
    );
  }

  writeFileSync(
    outputPath,
    JSON.stringify(
      {
        source: {
          name: "Hyperglot and GlyphsInfo",
          note: "Groups imitate the Glyphs sidebar; membership is derived and bounded by the codepoints Glyphs generates, except the legacy encodings",
        },
        groups,
      },
      null,
      2,
    ),
  );

  return groups.length;
}

/** Writes the groups read from a dump of Glyphs' own sidebar. */
function writeSidebarGroups(glyphs: readonly Glyph[], outputPath: string): number {
  const dump = JSON.parse(readFileSync(GLYPHS_SIDEBAR_PATH, "utf-8")) as GlyphsSidebarDump;

  const codepointByName = new Map<string, number>();
  for (const glyph of glyphs) {
    codepointByName.set(glyph.name, glyph.codepoint);
    for (const alias of glyph.altNames?.split(",") ?? []) {
      const name = alias.trim();
      if (name && !codepointByName.has(name)) codepointByName.set(name, glyph.codepoint);
    }
  }

  const { groups, problems } = groupsFromGlyphsDump(
    dump,
    (name) => codepointByName.get(name) ?? null,
    USER_DEFINED_ROWS,
  );
  for (const problem of problems) console.warn(`  groups: ${problem}`);

  writeFileSync(
    outputPath,
    JSON.stringify(
      { source: { name: "Glyphs", note: "Read from the Languages sidebar of Glyphs" }, groups },
      null,
      2,
    ),
  );

  return groups.length;
}

function categoryDrafts(
  glyphs: readonly Glyph[],
  glyphDataScripts: readonly string[],
): GroupDraft[] {
  const scripts = new Set(glyphDataScripts);
  const drafts: GroupDraft[] = [];

  for (const [category, name] of Object.entries(CATEGORY_GROUP_NAMES)) {
    drafts.push({
      name,
      basis: "derived",
      codepoints: glyphs
        .filter(
          (glyph) =>
            glyph.script !== null && scripts.has(glyph.script) && glyph.category === category,
        )
        .map((glyph) => glyph.codepoint),
    });
  }

  return drafts;
}

/** What Windows-1252 assigns to the bytes 0x80 to 0x9F; the five unassigned bytes are left out. */
const WINDOWS_1252_C1 = [
  0x20ac, 0x201a, 0x0192, 0x201e, 0x2026, 0x2020, 0x2021, 0x02c6, 0x2030, 0x0160, 0x2039, 0x0152,
  0x017d, 0x2018, 0x2019, 0x201c, 0x201d, 0x2022, 0x2013, 0x2014, 0x02dc, 0x2122, 0x0161, 0x203a,
  0x0153, 0x017e, 0x0178,
];

/**
 * Decodes every byte from the space character upwards, without the delete character.
 *
 * @remarks
 * Windows-1252 is spelled out because some Node builds decode that label as Latin-1.
 * Mac Roman comes from `TextDecoder`, and a known byte checks that the decoder is real.
 */
function decodeEncoding(label: "windows-1252" | "macintosh"): number[] {
  if (label === "windows-1252") {
    return [...range(0x20, 0x7e), ...WINDOWS_1252_C1, ...range(0xa0, 0xff)];
  }

  const decoder = new TextDecoder(label);
  if (decoder.decode(Uint8Array.of(0x80)) !== "Ä") {
    throw new Error("TextDecoder does not implement the macintosh encoding");
  }

  const codepoints: number[] = [];
  for (let byte = 0x20; byte <= 0xff; byte += 1) {
    if (byte === 0x7f) continue;

    codepoints.push(decoder.decode(Uint8Array.of(byte)).codePointAt(0) ?? 0xfffd);
  }

  return codepoints.filter((codepoint) => codepoint !== 0xfffd);
}

function range(from: number, to: number): number[] {
  return Array.from({ length: to - from + 1 }, (_, index) => from + index);
}

function slugify(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/(^-|-$)/g, "");
}
