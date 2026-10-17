import { useRef, useState } from "react";
import type { Language as LanguageEntry, LanguageCoverage } from "@shift/glyph-info";
import { SidebarRowButton } from "@/components/sidebar";
import type { LanguageGlyph } from "@/types/glyphCatalog";
import { MissingGlyphsPopover } from "./MissingGlyphsPopover";

export interface LanguageProps {
  coverage: LanguageCoverage;
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
  const { language, presentCount, requiredCount } = coverage;
  const rowRef = useRef<HTMLButtonElement>(null);
  const [glyphs, setGlyphs] = useState<LanguageGlyph[] | null>(null);

  return (
    <>
      <SidebarRowButton
        ref={rowRef}
        isActive={isActive}
        onClick={() => onSelect(language.id)}
        onContextMenu={(event) => {
          event.preventDefault();
          setGlyphs(languageGlyphs(language.id));
        }}
        title={languageTitle(language)}
      >
        <span className="min-w-0 flex-1 truncate pl-5 text-left">{language.name}</span>
        <span className="shrink-0 text-xs">
          {presentCount}/{requiredCount}
        </span>
      </SidebarRowButton>
      <MissingGlyphsPopover
        anchor={rowRef}
        languageName={language.name}
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
