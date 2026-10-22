import { execFileSync } from "node:child_process";
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { basename, join } from "node:path";
import { load } from "js-yaml";
import type { Language } from "../../src/types.js";
import type {
  HyperglotAttribute,
  HyperglotLanguage,
  HyperglotOrthography,
  HyperglotOrthographyStatus,
} from "../types/hyperglot.js";

const INCLUDED_LANGUAGE_STATUSES = new Set(["living", "constructed", "historical"]);
const INCLUDED_VALIDITIES = new Set(["draft", "preliminary", "verified"]);
const INCLUDED_ORTHOGRAPHY_STATUSES = new Set<HyperglotOrthographyStatus>(["primary", "secondary"]);
const ORTHOGRAPHY_STATUSES = new Set<HyperglotOrthographyStatus>([
  "primary",
  "secondary",
  "historical",
  "transliteration",
]);
const ATTRIBUTES = new Set<HyperglotAttribute>([
  "base",
  "auxiliary",
  "marks",
  "punctuation",
  "numerals",
  "currency",
]);
const REFERENCE_PATTERN = /<([^>]+)>/g;
const DOTTED_CIRCLE = 0x25cc;

export function generateLanguages(hyperglotPath: string, outputPath: string): number {
  const dataPath = join(hyperglotPath, "lib", "hyperglot", "data");
  const languages = new Map<string, HyperglotLanguage>();

  for (const file of readdirSync(dataPath)
    .filter((entry) => entry.endsWith(".yaml"))
    .sort()) {
    const language = parseLanguage(join(dataPath, file));
    languages.set(basename(file, ".yaml"), language);
  }

  languages.set(
    "default",
    parseLanguage(join(hyperglotPath, "lib", "hyperglot", "extra_data", "default.yaml")),
  );

  const generated: Language[] = [];
  for (const [languageCode, language] of languages) {
    if (languageCode === "default") continue;
    if (!INCLUDED_LANGUAGE_STATUSES.has(language.status ?? "living")) continue;
    if (!INCLUDED_VALIDITIES.has(language.validity ?? "todo")) continue;

    const includedOrthographies = (language.orthographies ?? []).filter((orthography) =>
      INCLUDED_ORTHOGRAPHY_STATUSES.has(orthography.status ?? "primary"),
    );

    // Identifiers of primary orthographies stay what they were before secondary
    // ones were included, because fonts store tracked language identifiers.
    for (const orthographyStatus of ["primary", "secondary"] as const) {
      const orthographies = includedOrthographies.filter(
        (orthography) => (orthography.status ?? "primary") === orthographyStatus,
      );
      const scriptCounts = countScripts(orthographies);
      const scriptIndices = new Map<string, number>();
      const statusSuffix = orthographyStatus === "primary" ? "" : `-${orthographyStatus}`;

      for (const orthography of orthographies) {
        const base = resolveAttribute(languages, languageCode, orthography, "base", new Set());
        const codepoints = uniqueCodepoints(base);
        if (codepoints.length === 0) continue;

        const scriptIndex = (scriptIndices.get(orthography.script) ?? 0) + 1;
        scriptIndices.set(orthography.script, scriptIndex);
        const scriptSuffix = slugify(orthography.script);
        const duplicateSuffix =
          (scriptCounts.get(orthography.script) ?? 0) > 1 ? `-${scriptIndex}` : "";

        generated.push({
          id: `${languageCode}-${scriptSuffix}${statusSuffix}${duplicateSuffix}`,
          name: language.preferred_name ?? language.name,
          autonym: orthography.autonym ?? null,
          script: orthography.script,
          baseCodepoints: codepoints,
          status: language.status ?? "living",
          validity: language.validity ?? "todo",
          orthography: orthographyStatus,
        });
      }
    }
  }

  disambiguateNames(generated);

  generated.sort(
    (left, right) =>
      left.script.localeCompare(right.script) ||
      left.name.localeCompare(right.name) ||
      left.id.localeCompare(right.id),
  );

  const revision = execFileSync("git", ["-C", hyperglotPath, "rev-parse", "HEAD"], {
    encoding: "utf-8",
  }).trim();
  writeFileSync(
    outputPath,
    JSON.stringify(
      {
        source: {
          name: "Hyperglot",
          url: "https://github.com/rosettatype/hyperglot",
          revision,
          license: "Apache-2.0",
        },
        languages: generated,
      },
      null,
      2,
    ),
  );

  return generated.length;
}

function parseLanguage(path: string): HyperglotLanguage {
  const parsed = load(readFileSync(path, "utf-8"));
  if (!parsed || typeof parsed !== "object") {
    throw new Error(`Invalid Hyperglot language record: ${path}`);
  }

  return parsed as HyperglotLanguage;
}

function countScripts(orthographies: HyperglotOrthography[]): Map<string, number> {
  const counts = new Map<string, number>();
  for (const orthography of orthographies) {
    counts.set(orthography.script, (counts.get(orthography.script) ?? 0) + 1);
  }
  return counts;
}

function resolveAttribute(
  languages: Map<string, HyperglotLanguage>,
  languageCode: string,
  orthography: HyperglotOrthography,
  attribute: HyperglotAttribute,
  resolving: Set<string>,
): string {
  const value = orthography[attribute] ?? "";
  const resolutionKey = `${languageCode}:${orthography.script}:${orthography.status ?? "primary"}:${attribute}`;
  if (resolving.has(resolutionKey)) {
    throw new Error(`Circular Hyperglot inheritance: ${resolutionKey}`);
  }

  resolving.add(resolutionKey);
  const resolved = value.replace(REFERENCE_PATTERN, (_match, reference: string) =>
    resolveReference(languages, orthography, attribute, reference, resolving),
  );
  resolving.delete(resolutionKey);
  return resolved;
}

function resolveReference(
  languages: Map<string, HyperglotLanguage>,
  sourceOrthography: HyperglotOrthography,
  sourceAttribute: HyperglotAttribute,
  reference: string,
  resolving: Set<string>,
): string {
  const [languageCode, ...qualifiers] = reference.trim().split(/\s+/);
  if (!languageCode) return "";

  const language = languages.get(languageCode);
  if (!language) throw new Error(`Unknown Hyperglot inheritance target: ${languageCode}`);

  let attribute = sourceAttribute;
  let status: HyperglotOrthographyStatus | null = null;
  const scriptParts: string[] = [];

  for (const qualifier of qualifiers) {
    if (ATTRIBUTES.has(qualifier as HyperglotAttribute)) {
      attribute = qualifier as HyperglotAttribute;
      continue;
    }
    if (ORTHOGRAPHY_STATUSES.has(qualifier as HyperglotOrthographyStatus)) {
      status = qualifier as HyperglotOrthographyStatus;
      continue;
    }
    scriptParts.push(qualifier);
  }

  const script = scriptParts.join(" ");
  const candidates = language.orthographies ?? [];
  const target =
    candidates.find(
      (candidate) =>
        (script === "" || candidate.script === script) &&
        (status === null || (candidate.status ?? "primary") === status) &&
        candidate.script === sourceOrthography.script,
    ) ??
    candidates.find(
      (candidate) =>
        (script === "" || candidate.script === script) &&
        (status === null || (candidate.status ?? "primary") === status),
    );

  if (!target) throw new Error(`Unresolved Hyperglot inheritance target: <${reference}>`);

  return resolveAttribute(languages, languageCode, target, attribute, resolving);
}

function uniqueCodepoints(value: string): number[] {
  const codepoints = new Set<number>();
  for (const character of value) {
    const codepoint = character.codePointAt(0);
    if (codepoint === undefined || codepoint === DOTTED_CIRCLE || /\s/u.test(character)) continue;
    codepoints.add(codepoint);
  }
  return codepoints
    .values()
    .toArray()
    .sort((left, right) => left - right);
}

function slugify(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/(^-|-$)/g, "");
}

const GEORGIAN_VARIANTS: readonly { name: string; test: (codepoint: number) => boolean }[] = [
  { name: "Asomtavruli", test: (codepoint) => codepoint >= 0x10a0 && codepoint <= 0x10cd },
  { name: "Nuskhuri", test: (codepoint) => codepoint >= 0x2d00 && codepoint <= 0x2d2d },
];

/**
 * Gives each orthography of a script its own name when several share one.
 *
 * @remarks
 * Hyperglot lists a language once per orthography, so Georgian would otherwise appear three times
 * and Bassari twice. The autonym tells orthographies apart when they differ; Georgian is told
 * apart by its historical alphabets; any other repeat is numbered.
 */
function disambiguateNames(languages: Language[]): void {
  const sharing = new Map<string, Language[]>();
  for (const language of languages) {
    const key = `${language.script}\u0000${language.name}`;
    sharing.set(key, [...(sharing.get(key) ?? []), language]);
  }

  for (const entries of sharing.values()) {
    if (entries.length < 2) continue;

    const autonyms = entries.map(({ autonym }) => autonym);
    const autonymsTell =
      autonyms.every((autonym) => autonym !== null) && new Set(autonyms).size === entries.length;

    entries.forEach((language, index) => {
      const variant = GEORGIAN_VARIANTS.find(({ test }) =>
        language.baseCodepoints.every((codepoint) => test(codepoint)),
      );
      if (language.script === "Georgian") {
        language.name = `${language.name} (${variant?.name ?? "Mkhedruli and Mtavruli"})`;
      } else if (autonymsTell) {
        language.name = `${language.name} (${language.autonym})`;
      } else {
        language.name = `${language.name} (${index + 1})`;
      }
    });
  }
}
