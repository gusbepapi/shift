import type { ReactNode } from "react";
import {
  isConstructedLanguage,
  isEuropeanLanguageId,
  type LanguageCoverage,
  type LanguageGroupCoverage,
  type LanguageScript as LanguageScriptGroup,
} from "@shift/glyph-info";
import { Collapsible, CollapsiblePanel, CollapsibleTrigger } from "@shift/ui";
import ChevronRightIcon from "@/assets/general/chevron-right.svg";
import { SidebarRowButton } from "@/components/sidebar";
import type { LanguageGlyph } from "@/types/glyphCatalog";
import { Language } from "./Language";
import { ScriptIcon } from "./ScriptIcon";

export interface LanguageScriptProps {
  group: LanguageScriptGroup;
  selectedLanguageId: string | null;
  canGenerate: boolean;
  onSelectLanguage: (languageId: string) => void;
  languageGlyphs: (languageId: string) => LanguageGlyph[];
  onGenerate: (codepoints: readonly number[]) => void;
}

export const LanguageScript = ({
  group,
  selectedLanguageId,
  canGenerate,
  onSelectLanguage,
  languageGlyphs,
  onGenerate,
}: LanguageScriptProps) => {
  const row = (coverage: LanguageCoverage | LanguageGroupCoverage) => {
    const id = "group" in coverage ? coverage.group.id : coverage.language.id;
    return (
      <Language
        key={id}
        coverage={coverage}
        isActive={selectedLanguageId === id}
        canGenerate={canGenerate}
        onSelect={onSelectLanguage}
        languageGlyphs={languageGlyphs}
        onGenerate={onGenerate}
      />
    );
  };

  // Groups with a section, such as «Character Sets», fold together; the rest stand alone
  const groupBlocks: { section: string | null; rows: LanguageGroupCoverage[] }[] = [];
  for (const coverage of group.groups) {
    const section = coverage.group.section;
    const last = groupBlocks[groupBlocks.length - 1];
    if (last && section !== null && last.section === section) last.rows.push(coverage);
    else groupBlocks.push({ section, rows: [coverage] });
  }

  const constructed = group.languages.filter(({ language }) => isConstructedLanguage(language));
  const natural = group.languages.filter(({ language }) => !isConstructedLanguage(language));
  const european = natural.filter(({ language }) => isEuropeanLanguageId(language.id));
  const others = natural.filter(({ language }) => !isEuropeanLanguageId(language.id));
  const languageSections = [
    { label: "European languages", languages: european },
    { label: "Other languages", languages: others },
    { label: "Constructed languages", languages: constructed },
  ].filter(({ languages }) => languages.length > 0);
  const labelled = group.groups.length > 0 || languageSections.length > 1;

  return (
    <Collapsible className="flex flex-col">
      <CollapsibleTrigger render={<SidebarRowButton />}>
        <div className="flex min-w-0 items-center gap-1">
          <ChevronRightIcon className="h-3 w-3 shrink-0 transition-transform duration-175 group-data-[panel-open]:rotate-90 group-data-[panel-closed]:rotate-0" />
          <ScriptIcon script={group.script} />
          <span className="truncate">{group.script}</span>
        </div>
      </CollapsibleTrigger>
      <CollapsiblePanel>
        <div className="flex flex-col gap-1 pt-1">
          {groupBlocks.map(({ section, rows }) =>
            section === null ? (
              rows.map(row)
            ) : (
              <Subsection key={`${section}-${rows[0]?.group.id}`} label={section}>
                {rows.map(row)}
              </Subsection>
            ),
          )}
          {languageSections.map(({ label, languages }) =>
            labelled ? (
              <Subsection key={label} label={label}>
                {languages.map(row)}
              </Subsection>
            ) : (
              languages.map(row)
            ),
          )}
        </div>
      </CollapsiblePanel>
    </Collapsible>
  );
};

/** A folder inside a script, closed until opened, such as «Romanization» or «European languages». */
const Subsection = ({ label, children }: { label: string; children: ReactNode }) => (
  <Collapsible className="flex flex-col">
    <CollapsibleTrigger render={<SidebarRowButton />}>
      <div className="flex min-w-0 items-center gap-1 pl-4">
        <ChevronRightIcon className="h-3 w-3 shrink-0 transition-transform duration-175 group-data-[panel-open]:rotate-90 group-data-[panel-closed]:rotate-0" />
        <span className="truncate">{label}</span>
      </div>
    </CollapsibleTrigger>
    <CollapsiblePanel>
      <div className="flex flex-col gap-1 pt-1">{children}</div>
    </CollapsiblePanel>
  </Collapsible>
);
