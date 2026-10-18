import { Fragment } from "react";
import {
  DEFAULT_SCRIPTS,
  isEuropeanLanguageId,
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
  const european = group.languages.filter(({ language }) => isEuropeanLanguageId(language.id));
  const others = group.languages.filter(({ language }) => !isEuropeanLanguageId(language.id));
  const sections = [
    { label: "European languages", languages: european },
    { label: "Other languages", languages: others },
  ].filter(({ languages }) => languages.length > 0);
  const labelled = group.groups.length > 0 || sections.length > 1;
  const defaultOpen = (DEFAULT_SCRIPTS as readonly string[]).includes(group.script);

  return (
    <Collapsible className="flex flex-col" defaultOpen={defaultOpen}>
      <CollapsibleTrigger render={<SidebarRowButton />}>
        <div className="flex min-w-0 items-center gap-1">
          <ChevronRightIcon className="h-3 w-3 shrink-0 transition-transform duration-175 group-data-[panel-open]:rotate-90 group-data-[panel-closed]:rotate-0" />
          <ScriptIcon script={group.script} />
          <span className="truncate">{group.script}</span>
        </div>
      </CollapsibleTrigger>
      <CollapsiblePanel>
        <div className="flex flex-col gap-1 pt-1">
          {group.groups.map((coverage, index) => (
            <Fragment key={coverage.group.id}>
              {coverage.group.section !== group.groups[index - 1]?.group.section &&
              coverage.group.section !== null ? (
                <SectionHeading label={coverage.group.section} />
              ) : null}
              <Language
                coverage={coverage}
                isActive={selectedLanguageId === coverage.group.id}
                canGenerate={canGenerate}
                onSelect={onSelectLanguage}
                languageGlyphs={languageGlyphs}
                onGenerate={onGenerate}
              />
            </Fragment>
          ))}
          {sections.map(({ label, languages }) => (
            <Fragment key={label}>
              {labelled ? <SectionHeading label={label} /> : null}
              {languages.map((coverage) => (
                <Language
                  key={coverage.language.id}
                  coverage={coverage}
                  isActive={selectedLanguageId === coverage.language.id}
                  canGenerate={canGenerate}
                  onSelect={onSelectLanguage}
                  languageGlyphs={languageGlyphs}
                  onGenerate={onGenerate}
                />
              ))}
            </Fragment>
          ))}
        </div>
      </CollapsiblePanel>
    </Collapsible>
  );
};

const SectionHeading = ({ label }: { label: string }) => (
  <div className="truncate pl-7 pt-1 text-xs font-medium text-muted">{label}</div>
);
