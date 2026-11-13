import { useMemo, useRef, useState } from "react";
import type { LanguageScript } from "@shift/glyph-info";
import {
  Button,
  Checkbox,
  Input,
  Popover,
  PopoverClose,
  PopoverPopup,
  PopoverPortal,
  PopoverPositioner,
  PopoverTitle,
  PopoverTrigger,
  Search,
  Tooltip,
  X,
} from "@shift/ui";
import PlusIcon from "@/assets/general/plus.svg";
import { useSidebarPanelAnchor } from "@/hooks/useSidebarPanelAnchor";
import { SidebarActionButton } from "@/components/sidebar";
import { useCheckboxRange } from "@/hooks/useCheckboxRange";
import { listedIds, listedLanguages, scriptEntryIds } from "./languageText";
import { ScriptIcon } from "./ScriptIcon";
import { NameQuery } from "@/lib/utils/NameQuery";
import { useMemberSet } from "@/hooks/useMemberSet";

export interface LanguagePickerProps {
  scripts: readonly LanguageScript[];
  trackedLanguageIds: readonly string[];
  disabled: boolean;
  onApply: (languageIds: readonly string[]) => void;
}

/** How many of a script's character sets and listed languages the font has completely. */
function fullySupportedCount(script: LanguageScript): number {
  const characterSets = script.characterSets.filter(
    ({ presentCount, requiredCount }) => presentCount === requiredCount,
  );
  const languages = listedLanguages(script).filter(
    ({ fullPresentCount, fullRequiredCount }) => fullPresentCount === fullRequiredCount,
  );

  return characterSets.length + languages.length;
}

/**
 * Chooses which scripts the sidebar tracks; ticking a script tracks all of its
 * character sets and listed languages, and a script with only some tracked shows a dash.
 * Shift-click applies the clicked row's new state to the range from
 * the last clicked row.
 */
export const LanguagePicker = ({
  scripts,
  trackedLanguageIds,
  disabled,
  onApply,
}: LanguagePickerProps) => {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const checked = useMemberSet<string>();
  const checkedIds = checked.values;
  const searchRef = useRef<HTMLInputElement>(null);

  const visibleScripts = useMemo(() => {
    const search = NameQuery.from(query);
    return search.isEmpty ? scripts : search.filterBy(scripts, ({ script }) => [script]);
  }, [query, scripts]);

  const { onMouseDown, itemsToChange, resetRange } = useCheckboxRange(visibleScripts);

  const handleOpenChange = (nextOpen: boolean) => {
    if (nextOpen) {
      setQuery("");
      checked.clear();
      checked.setMany(listedIds(scripts, trackedLanguageIds), true);
      resetRange();
    }
    setOpen(nextOpen);
  };

  const setScriptChecked = (target: LanguageScript, isChecked: boolean) => {
    const entryIds = itemsToChange(target).flatMap(scriptEntryIds);
    checked.setMany(entryIds, isChecked);
  };

  const apply = () => {
    const tracked = listedIds(scripts, trackedLanguageIds);
    const previous = new Set(tracked);
    onApply([
      ...tracked.filter((languageId) => checkedIds.has(languageId)),
      ...[...checkedIds].filter((languageId) => !previous.has(languageId)),
    ]);
    setOpen(false);
  };

  const panelAnchor = useSidebarPanelAnchor();

  if (disabled) {
    return (
      <Tooltip content="Add languages">
        <SidebarActionButton label="Add languages" aria-disabled="true">
          <PlusIcon className="h-3 w-3" />
        </SidebarActionButton>
      </Tooltip>
    );
  }

  return (
    <Popover open={open} onOpenChange={handleOpenChange} modal={false}>
      <Tooltip content="Add languages">
        <PopoverTrigger
          ref={panelAnchor.triggerRef}
          render={
            <SidebarActionButton label="Add languages">
              <PlusIcon className="h-3 w-3" />
            </SidebarActionButton>
          }
        />
      </Tooltip>
      <PopoverPortal>
        <PopoverPositioner {...panelAnchor.positioner}>
          <PopoverPopup className="flex w-64 flex-col p-0" initialFocus={searchRef}>
            <div className="flex h-8 items-center justify-between border-b border-line-subtle px-2">
              <PopoverTitle>Scripts</PopoverTitle>
              <Tooltip content="Close">
                <PopoverClose variant="icon" aria-label="Close">
                  <X className="h-4 w-4" />
                </PopoverClose>
              </Tooltip>
            </div>

            <div className="p-2">
              <Input
                ref={searchRef}
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder="Search scripts..."
                aria-label="Search scripts"
                size="md"
                icon={<Search className="h-3 w-3 text-muted" />}
                iconPosition="left"
              />
            </div>

            <div
              role="group"
              aria-label="Scripts to track"
              className="max-h-80 overflow-y-auto px-2 pb-2"
            >
              {visibleScripts.length === 0 ? (
                <div className="px-1 py-4 text-center text-ui text-muted">No scripts found</div>
              ) : (
                visibleScripts.map((script) => {
                  const entryIds = scriptEntryIds(script);
                  const entryCount = entryIds.length;
                  const trackedCount = entryIds.filter((entryId) => checkedIds.has(entryId)).length;
                  const allTracked = entryCount > 0 && trackedCount === entryCount;
                  const someTracked = trackedCount > 0 && !allTracked;
                  const supportedCount = fullySupportedCount(script);

                  return (
                    <label
                      key={script.script}
                      className="flex cursor-pointer select-none items-center gap-2 rounded px-1 py-1.5 text-ui text-primary hover:bg-hover"
                      title={`${supportedCount} of ${entryCount} character sets and languages fully supported`}
                      onMouseDown={onMouseDown}
                    >
                      <Checkbox
                        checked={allTracked}
                        indeterminate={someTracked}
                        onCheckedChange={(checked) => setScriptChecked(script, checked)}
                      />
                      <ScriptIcon script={script.script} />
                      <span className="min-w-0 flex-1 truncate">{script.script}</span>
                      {trackedCount > 0 && (
                        <span className="shrink-0 text-xs text-muted">
                          {trackedCount}/{entryCount}
                        </span>
                      )}
                    </label>
                  );
                })
              )}
            </div>

            <div className="grid grid-cols-2 gap-2 border-t border-line-subtle p-2">
              <PopoverClose
                render={
                  <Button type="button" className="h-7.5 text-ui">
                    Cancel
                  </Button>
                }
              />
              <Button type="button" variant="primary" className="h-7.5 text-ui" onClick={apply}>
                Done
              </Button>
            </div>
          </PopoverPopup>
        </PopoverPositioner>
      </PopoverPortal>
    </Popover>
  );
};
