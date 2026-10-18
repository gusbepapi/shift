import { useRef, useState } from "react";
import type {
  Language as LanguageEntry,
  LanguageCoverage,
  LanguageGroup,
  LanguageGroupCoverage,
} from "@shift/glyph-info";
import { SidebarRowButton } from "@/components/sidebar";
import type { LanguageGlyph } from "@/types/glyphCatalog";
import { MissingGlyphsPopover } from "./MissingGlyphsPopover";

export interface LanguageProps {
  coverage: LanguageCoverage | LanguageGroupCoverage;
  isActive: boolean;
  canGenerate: boolean;
  onSelect: (languageId: string) => void;
  languageGlyphs: (languageId: string) => LanguageGlyph[];
  onGenerate: (codepoints: readonly number[]) => void;
}

export const Language = ({
  coverage,
  isActive,
  canGenerate,
  onSelect,
  languageGlyphs,
  onGenerate,
}: LanguageProps) => {
  const { presentCount, requiredCount } = coverage;
  const entry = "group" in coverage ? coverage.group : coverage.language;
  const title = "group" in coverage ? groupTitle(coverage.group) : languageTitle(coverage.language);
  const complete = requiredCount > 0 && presentCount === requiredCount;
  const rowRef = useRef<HTMLButtonElement>(null);
  const [glyphs, setGlyphs] = useState<LanguageGlyph[] | null>(null);

  return (
    <>
      <SidebarRowButton
        ref={rowRef}
        isActive={isActive}
        onClick={() => onSelect(entry.id)}
        onContextMenu={(event) => {
          event.preventDefault();
          setGlyphs(languageGlyphs(entry.id));
        }}
        title={title}
      >
        <span className="min-w-0 flex-1 truncate pl-5 text-left">{entry.name}</span>
        <span className="shrink-0 text-xs">
          {complete ? "✓" : `${presentCount}/${requiredCount}`}
        </span>
      </SidebarRowButton>
      <MissingGlyphsPopover
        anchor={rowRef}
        languageName={entry.name}
        glyphs={glyphs}
        canGenerate={canGenerate}
        onClose={() => setGlyphs(null)}
        onGenerate={onGenerate}
      />
    </>
  );
};

/** Tooltip text: the autonym followed by anything that qualifies how reliable or current the entry is. */
export function languageTitle(language: LanguageEntry): string | undefined {
  const qualifiers: string[] = [];
  if (language.status === "historical") qualifiers.push("Historical");
  if (language.status === "constructed") qualifiers.push("Constructed");
  if (language.orthography === "secondary") qualifiers.push("Secondary orthography");
  if (language.validity === "draft") qualifiers.push("Draft data");

  const parts = language.autonym === null ? qualifiers : [language.autonym, ...qualifiers];
  return parts.length === 0 ? undefined : parts.join(" · ");
}

/** Tooltip text of a group, saying whether its characters are published or derived. */
export function groupTitle(group: LanguageGroup): string {
  return group.basis === "exact"
    ? "Published character set"
    : "Derived from Hyperglot and GlyphData";
}
