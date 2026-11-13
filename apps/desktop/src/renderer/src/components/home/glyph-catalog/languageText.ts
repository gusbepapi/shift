import {
  isConstructedLanguage,
  isEuropeanLanguageId,
  type Language,
  type LanguageCoverage,
  type LanguageGroup,
  type LanguageScript,
} from "@shift/glyph-info";
import type { CharacterSetBlock, LanguageSection } from "@/types/glyphCatalog";

/** The languages listed in their own right; the rest fold under one of them. */
export function listedLanguages(script: LanguageScript): LanguageCoverage[] {
  return script.languages.filter(({ language }) => language.coveredBy === null);
}

/**
 * Tracked ids with each folded language replaced by the one covering it, so a
 * font that tracks Algerian Arabic shows Standard Arabic ticked.
 */
export function listedIds(
  scripts: readonly LanguageScript[],
  trackedLanguageIds: readonly string[],
): string[] {
  const coverOf = new Map(
    scripts.flatMap(({ languages }) =>
      languages.map(({ language }) => [language.id, language.coveredBy ?? language.id] as const),
    ),
  );
  return [...new Set(trackedLanguageIds.map((id) => coverOf.get(id) ?? id))];
}

/** Every name a language row answers to in search: its own, its autonym, and what it covers. */
export function searchableNames({ language, covers, varieties }: LanguageCoverage): string[] {
  return [
    language.name,
    ...(language.autonym ? [language.autonym] : []),
    ...covers.map(({ name }) => name),
    ...varieties.map(({ language: variety }) => variety.name),
  ];
}

/** Identifiers of every character set and listed language of a script, as the pickers tick them. */
export function scriptEntryIds(script: LanguageScript): string[] {
  return [
    ...script.characterSets.map(({ group }) => group.id),
    ...listedLanguages(script).map(({ language }) => language.id),
  ];
}

/**
 * A script's character sets as the sidebar files them: those sharing a section, such as
 * «Character Sets», fold together, and the rest stand alone.
 */
export function characterSetBlocks(script: LanguageScript): CharacterSetBlock[] {
  const blocks: CharacterSetBlock[] = [];
  for (const coverage of script.characterSets) {
    const section = coverage.group.section;
    const last = blocks.at(-1);
    if (last && section !== null && last.section === section) last.rows.push(coverage);
    else blocks.push({ section, rows: [coverage] });
  }

  return blocks;
}

/** A script's languages by kind, European first and constructed last; empty kinds are left out. */
export function languageSections(script: LanguageScript): LanguageSection[] {
  const constructed = script.languages.filter(({ language }) => isConstructedLanguage(language));
  const natural = script.languages.filter(({ language }) => !isConstructedLanguage(language));
  const european = natural.filter(({ language }) => isEuropeanLanguageId(language.id));
  const others = natural.filter(({ language }) => !isEuropeanLanguageId(language.id));

  return [
    { label: "European languages", languages: european },
    { label: "Other languages", languages: others },
    { label: "Constructed languages", languages: constructed },
  ].filter(({ languages }) => languages.length > 0);
}

/** What qualifies how current or reliable a language entry is: "Historical · Draft data". */
function languageQualifiers(language: Language): string[] {
  const qualifiers: string[] = [];
  if (language.status === "historical") qualifiers.push("Historical");
  if (language.status === "constructed") qualifiers.push("Constructed");
  if (language.orthography === "secondary") qualifiers.push("Secondary orthography");
  if (language.validity === "draft") qualifiers.push("Draft data");

  return qualifiers;
}

/** Tooltip text of a character set, saying whether its characters are published or derived. */
export function characterSetTooltip(group: LanguageGroup): string {
  switch (group.basis) {
    case "exact":
      return "Published character set";
    case "custom":
      return "Custom character set";
    case "derived":
      return "Derived from Hyperglot and GlyphData";
  }
}

/**
 * A language row's tooltip: its autonym with anything that qualifies the
 * entry, its full support, since the row counts only what it adds beyond
 * Basic, and the languages folded under it.
 */
export function languageTooltip({
  language,
  fullPresentCount,
  fullRequiredCount,
  covers,
  varieties,
}: LanguageCoverage): string {
  const heading = [
    ...(language.autonym ? [language.autonym] : []),
    ...languageQualifiers(language),
  ];
  const lines = [
    ...(heading.length > 0 ? [heading.join(" · ")] : []),
    `Supports ${language.name} fully: ${fullPresentCount}/${fullRequiredCount}`,
  ];
  if (covers.length > 0) lines.push(`Also covers ${covers.map(({ name }) => name).join(", ")}`);
  if (varieties.length > 0) {
    const listed = varieties.map(({ language: variety, extraCount }) => {
      return `${variety.name} (+${extraCount})`;
    });
    lines.push(`Varieties needing more: ${listed.join(", ")}`);
  }
  return lines.join("\n");
}
