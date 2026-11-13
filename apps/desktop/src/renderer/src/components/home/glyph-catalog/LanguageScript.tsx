import type { ReactNode } from "react";
import type {
  LanguageCoverage,
  LanguageGroupCoverage,
  LanguageScript as LanguageScriptGroup,
  RequiredGlyph,
  ScriptGroupCoverage,
} from "@shift/glyph-info";
import { Collapsible, CollapsiblePanel, CollapsibleTrigger } from "@shift/ui";
import ChevronRightIcon from "@/assets/general/chevron-right.svg";
import { SidebarActionSlot, SidebarRowButton } from "@/components/sidebar";
import type { LanguageGlyph } from "@/types/glyphCatalog";
import { Language } from "./Language";
import {
  characterSetBlocks,
  characterSetTooltip,
  languageSections,
  languageTooltip,
} from "./languageText";
import { ScriptIcon } from "./ScriptIcon";
import { ScriptLanguagePicker } from "./ScriptLanguagePicker";

export interface LanguageScriptProps {
  group: LanguageScriptGroup;
  /** Every language written in the script, for adding more of them. */
  allLanguages: LanguageScriptGroup;
  trackedLanguageIds: readonly string[];
  onTrackLanguages: (languageIds: readonly string[]) => void;
  selectedLanguageId: string | null;
  canGenerate: boolean;
  onSelectLanguage: (setId: string) => void;
  languageGlyphs: (setId: string) => LanguageGlyph[];
  onGenerate: (glyphs: readonly RequiredGlyph[]) => void;
}

export const LanguageScript = ({
  group,
  allLanguages,
  trackedLanguageIds,
  onTrackLanguages,
  selectedLanguageId,
  canGenerate,
  onSelectLanguage,
  languageGlyphs,
  onGenerate,
}: LanguageScriptProps) => {
  const row = (setId: string, name: string, counts: RowCounts, title?: string) => (
    <Language
      key={setId}
      setId={setId}
      name={name}
      title={title}
      presentCount={counts.presentCount}
      requiredCount={counts.requiredCount}
      isActive={selectedLanguageId === setId}
      canGenerate={canGenerate}
      onSelect={onSelectLanguage}
      languageGlyphs={languageGlyphs}
      onGenerate={onGenerate}
    />
  );
  const scriptGroupRow = (coverage: ScriptGroupCoverage) =>
    row(coverage.group.id, coverage.group.name, coverage);
  const characterSetRow = (coverage: LanguageGroupCoverage) =>
    row(coverage.group.id, coverage.group.name, coverage, characterSetTooltip(coverage.group));
  const languageRow = (coverage: LanguageCoverage) =>
    row(coverage.language.id, coverage.language.name, coverage, languageTooltip(coverage));

  const blocks = characterSetBlocks(group);
  const sections = languageSections(group);
  // Languages are filed under headings once character sets or several kinds of language share the script
  const labelled = group.characterSets.length > 0 || sections.length > 1;

  return (
    <Collapsible className="flex flex-col">
      {/* One hover background for the row and its +, as SidebarActionRow draws it. */}
      <div className="group flex h-7 w-full min-w-0 items-center rounded transition-colors hover:bg-hover/50">
        <CollapsibleTrigger
          render={<SidebarRowButton variant="transparent" className="w-auto min-w-0 flex-1" />}
        >
          <div className="flex min-w-0 items-center gap-1">
            <ChevronRightIcon className="h-3 w-3 shrink-0 transition-transform duration-175 group-data-[panel-open]:rotate-90 group-data-[panel-closed]:rotate-0" />
            <ScriptIcon script={group.script} />
            <span className="truncate">{group.script}</span>
          </div>
        </CollapsibleTrigger>
        <SidebarActionSlot>
          <ScriptLanguagePicker
            script={allLanguages}
            trackedLanguageIds={trackedLanguageIds}
            disabled={!canGenerate}
            onApply={onTrackLanguages}
          />
        </SidebarActionSlot>
      </div>
      <CollapsiblePanel>
        <div className="flex flex-col gap-1 pt-1">
          {group.groups.map(scriptGroupRow)}
          {blocks.map(({ section, rows }) =>
            section === null ? (
              rows.map(characterSetRow)
            ) : (
              <Subsection key={`${section}-${rows[0]?.group.id}`} label={section}>
                {rows.map(characterSetRow)}
              </Subsection>
            ),
          )}
          {sections.map(({ label, languages }) =>
            labelled ? (
              <Subsection key={label} label={label}>
                {languages.map(languageRow)}
              </Subsection>
            ) : (
              languages.map(languageRow)
            ),
          )}
        </div>
      </CollapsiblePanel>
    </Collapsible>
  );
};

/** The two counts every coverage carries, whatever it covers. */
interface RowCounts {
  presentCount: number;
  requiredCount: number;
}

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
