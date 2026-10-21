import { useEffect, useRef, useState } from "react";
import { useSignalState } from "@shift/editor/signals";
import {
  addColorLayer,
  addPalette,
  addPaletteEntry,
  colorLayersOf,
  cssColor,
  removeColorLayer,
  removePalette,
  removePaletteEntry,
  renamePalette,
  resolvePaintColor,
  setLayerSolid,
  setPaletteColor,
  type ColorLayer,
} from "@shift/editor";
import type { ColorData, GlyphId, PaintColor, Rgba } from "@shift/types";
import { Button, Input, Minus, Plus, Toggle, ToggleGroup, Tooltip, X } from "@shift/ui";
import { useEditor } from "@/workspace/WorkspaceContext";
import { SidebarNumberField } from "./SidebarNumberField";
import { SidebarSection, SidebarSubsection } from "./SidebarSection";

const FIRST_COLOUR: Rgba = { r: 214, g: 40, b: 57, a: 255 };

/**
 * The font's colour palettes and, for the shown glyph, its colour layers, with edits.
 *
 * @remarks
 * Every edit composes the complete next colour data with the pure functions in `@shift/editor`
 * and submits it as one undoable `setColor` edit, so Rust validates the whole document. An edit
 * those functions refuse, such as removing an entry a layer paints with, is explained under the
 * section instead of being sent. Layers are listed only when the scene holds exactly one glyph,
 * as the Glyph section above does.
 */
export const GlyphColourSection = () => {
  const editor = useEditor();
  const color = useSignalState(editor.font.colorCell);
  const scene = useSignalState(editor.scene.cell);
  const [paletteIndex, setPaletteIndex] = useState(0);
  const [entryIndex, setEntryIndex] = useState(0);
  const [notice, setNotice] = useState<string | null>(null);
  const disabled = editor.sessionMode !== "workspace";

  const commit = (next: ColorData | null, refusal: string): boolean => {
    if (!next) {
      setNotice(refusal);
      return false;
    }
    setNotice(null);
    editor.font.setColor(next);
    return true;
  };

  if (color.palettes.length === 0) {
    if (disabled) return null;
    return (
      <SidebarSection title="Colour">
        <Button
          variant="default"
          size="sm"
          onClick={() => commit(addPaletteEntry(color, FIRST_COLOUR), "")}
        >
          Add colour palette
        </Button>
      </SidebarSection>
    );
  }

  const activePalette = Math.min(paletteIndex, color.palettes.length - 1);
  const palette = color.palettes[activePalette];
  const activeEntry = Math.min(entryIndex, Math.max(palette.colors.length - 1, 0));
  const entry = palette.colors[activeEntry];
  const glyphNodes = scene.nodes.filter((node) => node.kind === "glyph");
  const node = glyphNodes.length === 1 ? glyphNodes[0] : undefined;
  const glyphId = node?.kind === "glyph" ? node.glyphId : null;
  const layers = glyphId ? colorLayersOf(color, glyphId) : [];

  return (
    <SidebarSection title="Colour">
      <SidebarSubsection title="Palette">
        <div className="flex items-center gap-1">
          <ToggleGroup
            aria-label="Palette"
            className="min-w-0 flex-1 flex-wrap"
            value={[String(activePalette)]}
            onValueChange={(pressed) => {
              const next = Number(pressed.at(0));
              if (Number.isInteger(next)) setPaletteIndex(next);
            }}
          >
            {color.palettes.map((candidate, index) => (
              <Toggle
                key={index}
                value={String(index)}
                aria-label={candidate.name ?? `Palette ${index + 1}`}
                title={candidate.name ?? `Palette ${index + 1}`}
              >
                {index + 1}
              </Toggle>
            ))}
          </ToggleGroup>
          <IconAction
            label="Add palette"
            disabled={disabled}
            onClick={() => {
              if (commit(addPalette(color), "")) setPaletteIndex(color.palettes.length);
            }}
          >
            <Plus className="size-3.5" />
          </IconAction>
          <IconAction
            label="Remove palette"
            disabled={disabled}
            onClick={() => {
              const next = removePalette(color, activePalette);
              if (commit(next, "A colour glyph needs at least one palette.")) {
                setPaletteIndex(Math.max(0, activePalette - 1));
              }
            }}
          >
            <Minus className="size-3.5" />
          </IconAction>
        </div>
        <CommitInput
          ariaLabel="Palette name"
          placeholder={`Palette ${activePalette + 1}`}
          value={palette.name ?? ""}
          disabled={disabled}
          onCommit={(name) => commit(renamePalette(color, activePalette, name), "")}
        />
      </SidebarSubsection>

      <SidebarSubsection title="Entries">
        <div className="flex items-start gap-1">
          <ToggleGroup
            aria-label="Palette entries"
            className="min-w-0 flex-1 flex-wrap"
            value={[String(activeEntry)]}
            onValueChange={(pressed) => {
              const next = Number(pressed.at(0));
              if (Number.isInteger(next)) setEntryIndex(next);
            }}
          >
            {palette.colors.map((candidate, index) => (
              <Toggle
                key={index}
                value={String(index)}
                aria-label={`Entry ${index}, ${hex(candidate)}`}
                title={`Entry ${index}, ${hex(candidate)}`}
              >
                <Swatch colour={candidate} />
              </Toggle>
            ))}
          </ToggleGroup>
          <IconAction
            label="Add entry"
            disabled={disabled}
            onClick={() => {
              if (commit(addPaletteEntry(color, entry ?? FIRST_COLOUR), "")) {
                setEntryIndex(palette.colors.length);
              }
            }}
          >
            <Plus className="size-3.5" />
          </IconAction>
          <IconAction
            label="Remove entry"
            disabled={disabled || !entry}
            onClick={() => {
              const next = removePaletteEntry(color, activeEntry);
              commit(next, `Entry ${activeEntry} is painted by a layer, so it stays.`);
            }}
          >
            <Minus className="size-3.5" />
          </IconAction>
        </div>
        {entry ? (
          <div className="flex items-center gap-2">
            <ColourInput
              ariaLabel={`Colour of entry ${activeEntry}`}
              value={entry}
              disabled={disabled}
              onCommit={(rgb) =>
                commit(
                  setPaletteColor(color, activePalette, activeEntry, { ...rgb, a: entry.a }),
                  "",
                )
              }
            />
            <div className="min-w-0 flex-1">
              <SidebarNumberField
                ariaLabel={`Opacity of entry ${activeEntry}`}
                label="A"
                suffix="%"
                value={Math.round((entry.a / 255) * 100)}
                disabled={disabled}
                onValueCommit={(percent) => {
                  const a = Math.round((Math.min(100, Math.max(0, percent)) / 100) * 255);
                  commit(setPaletteColor(color, activePalette, activeEntry, { ...entry, a }), "");
                }}
              />
            </div>
          </div>
        ) : null}
      </SidebarSubsection>

      {glyphId ? (
        <SidebarSubsection title="Layers">
          {layers.length > 0 ? (
            <ol className="flex flex-col gap-2" aria-label="Colour layers, bottom first">
              {layers.map((layer, index) => (
                <LayerRow
                  key={index}
                  layer={layer}
                  palette={palette.colors}
                  disabled={disabled}
                  onEntry={(entryIndex) =>
                    commit(setLayerSolid(color, glyphId, index, { entryIndex }), "")
                  }
                  onAlpha={(alpha) => commit(setLayerSolid(color, glyphId, index, { alpha }), "")}
                  onRemove={() => commit(removeColorLayer(color, glyphId, index), "")}
                />
              ))}
            </ol>
          ) : null}
          <AddLayer
            color={color}
            glyphId={glyphId}
            disabled={disabled}
            onAdd={(next, refusal) => commit(next, refusal)}
          />
        </SidebarSubsection>
      ) : null}
      {notice ? (
        <p role="status" className="text-ui text-muted">
          {notice}
        </p>
      ) : null}
    </SidebarSection>
  );
};

const FILL_LABELS: Record<ColorLayer["fill"]["kind"], string> = {
  solid: "Solid",
  linearGradient: "Linear gradient",
  radialGradient: "Radial gradient",
};

const LayerRow = ({
  layer,
  palette,
  disabled,
  onEntry,
  onAlpha,
  onRemove,
}: {
  readonly layer: ColorLayer;
  readonly palette: readonly Rgba[];
  readonly disabled: boolean;
  readonly onEntry: (entryIndex: number) => void;
  readonly onAlpha: (alpha: number) => void;
  readonly onRemove: () => void;
}) => {
  const editor = useEditor();
  const name = editor.font.entryForId(layer.glyph)?.name ?? layer.glyph;
  const { fill } = layer;

  return (
    <li className="flex flex-col gap-1 text-ui text-secondary">
      <div className="flex items-center gap-2">
        <span className="min-w-0 flex-1 truncate text-primary">{name}</span>
        <span className="shrink-0">{FILL_LABELS[fill.kind]}</span>
        <IconAction label={`Remove layer ${name}`} disabled={disabled} onClick={onRemove}>
          <X className="size-3.5" />
        </IconAction>
      </div>
      {fill.kind === "solid" ? (
        <div className="flex items-center gap-2">
          <ToggleGroup
            aria-label={`Colour of layer ${name}`}
            className="min-w-0 flex-1 flex-wrap"
            value={[fill.color.kind === "palette" ? String(fill.color.index) : ""]}
            disabled={disabled}
            onValueChange={(pressed) => {
              const next = Number(pressed.at(0));
              if (pressed.length > 0 && Number.isInteger(next)) onEntry(next);
            }}
          >
            {palette.map((candidate, index) => (
              <Toggle
                key={index}
                value={String(index)}
                aria-label={`Entry ${index}, ${hex(candidate)}`}
                title={`Entry ${index}, ${hex(candidate)}`}
              >
                <Swatch colour={candidate} />
              </Toggle>
            ))}
          </ToggleGroup>
          <div className="w-16 shrink-0">
            <SidebarNumberField
              ariaLabel={`Opacity of layer ${name}`}
              suffix="%"
              value={Math.round(fill.alpha * 100)}
              disabled={disabled}
              onValueCommit={(percent) => onAlpha(Math.min(100, Math.max(0, percent)) / 100)}
            />
          </div>
        </div>
      ) : (
        <span className="flex gap-0.5">
          {fill.stops.map((stop, index) => (
            <StopSwatch key={index} color={stop.color} alpha={stop.alpha} palette={palette} />
          ))}
        </span>
      )}
    </li>
  );
};

/** Adds a solid layer painted with the first entry, named by the layer glyph's name. */
const AddLayer = ({
  color,
  glyphId,
  disabled,
  onAdd,
}: {
  readonly color: ColorData;
  readonly glyphId: GlyphId;
  readonly disabled: boolean;
  readonly onAdd: (next: ColorData | null, refusal: string) => boolean;
}) => {
  const editor = useEditor();
  return (
    <CommitInput
      ariaLabel="Add layer glyph"
      placeholder="Add layer: glyph name"
      value=""
      clearOnCommit
      disabled={disabled}
      onCommit={(name) => {
        if (!name) return;
        const layer = editor.font.entryForName(name);
        if (!layer) {
          onAdd(null, `There is no glyph named ${name}.`);
          return;
        }
        onAdd(
          addColorLayer(color, glyphId, layer.id, 0),
          `${name} cannot be a layer here: a layer must differ from its base, must not be a colour glyph itself, and the base must not be a layer elsewhere.`,
        );
      }}
    />
  );
};

const IconAction = ({
  label,
  disabled,
  onClick,
  children,
}: {
  readonly label: string;
  readonly disabled: boolean;
  readonly onClick: () => void;
  readonly children: React.ReactNode;
}) => (
  <Tooltip content={label}>
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label={label}
      aria-disabled={disabled || undefined}
      onClick={() => {
        if (!disabled) onClick();
      }}
    >
      {children}
    </Button>
  </Tooltip>
);

/** A text field that commits on Enter or blur and leaves editor shortcuts alone while focused. */
const CommitInput = ({
  ariaLabel,
  placeholder,
  value,
  disabled,
  clearOnCommit = false,
  onCommit,
}: {
  readonly ariaLabel: string;
  readonly placeholder: string;
  readonly value: string;
  readonly disabled: boolean;
  readonly clearOnCommit?: boolean;
  readonly onCommit: (value: string) => void;
}) => {
  const [draft, setDraft] = useState<string | null>(null);
  const finish = () => {
    if (draft === null) return;
    const next = draft;
    setDraft(null);
    if (clearOnCommit || next !== value) onCommit(next);
  };
  return (
    <Input
      aria-label={ariaLabel}
      placeholder={placeholder}
      value={draft ?? value}
      disabled={disabled}
      onChange={(event) => setDraft(event.currentTarget.value)}
      onBlur={finish}
      onKeyDown={(event) => {
        event.nativeEvent.stopImmediatePropagation();
        if (event.key === "Enter") event.currentTarget.blur();
        if (event.key === "Escape") {
          setDraft(null);
          event.currentTarget.blur();
        }
      }}
    />
  );
};

/**
 * A native colour input that commits when the picker closes, not on every drag step, so one
 * choice is one undo step.
 */
const ColourInput = ({
  ariaLabel,
  value,
  disabled,
  onCommit,
}: {
  readonly ariaLabel: string;
  readonly value: Rgba;
  readonly disabled: boolean;
  readonly onCommit: (rgb: Pick<Rgba, "r" | "g" | "b">) => void;
}) => {
  const ref = useRef<HTMLInputElement>(null);
  const commitRef = useRef(onCommit);
  commitRef.current = onCommit;
  const hexValue = hex({ ...value, a: 255 });

  useEffect(() => {
    const input = ref.current;
    if (!input) return;
    const onChange = (event: Event) => {
      const { value } = event.target as HTMLInputElement;
      const parsed = parseHex(value);
      if (parsed) commitRef.current(parsed);
    };
    input.addEventListener("change", onChange);
    return () => input.removeEventListener("change", onChange);
  }, [hexValue]);

  return (
    <input
      // Remounting on a changed value shows an undo, redo, or opacity echo in the picker.
      key={hexValue}
      ref={ref}
      type="color"
      aria-label={ariaLabel}
      defaultValue={hexValue}
      disabled={disabled}
      className="h-6 w-8 shrink-0 cursor-pointer rounded-sm border border-line-subtle bg-transparent p-0 disabled:opacity-50"
    />
  );
};

const StopSwatch = ({
  color,
  alpha,
  palette,
}: {
  readonly color: PaintColor;
  readonly alpha: number;
  readonly palette: readonly Rgba[];
}) => {
  const resolved = resolvePaintColor(color, palette);
  if (!resolved) return <Swatch colour={null} />;
  return <Swatch colour={resolved} alpha={alpha} />;
};

/** A small colour chip; a `null` colour draws the text foreground as a hatched chip. */
const Swatch = ({
  colour,
  alpha = 1,
}: {
  readonly colour: Rgba | null;
  readonly alpha?: number;
}) => (
  <span
    aria-hidden
    className="block size-4 rounded-sm border border-line-subtle"
    style={
      colour
        ? { backgroundColor: cssColor(colour, alpha) }
        : {
            backgroundImage:
              "repeating-linear-gradient(45deg, currentColor 0 1px, transparent 1px 4px)",
          }
    }
  />
);

function hex(colour: Rgba): string {
  const channel = (value: number) => value.toString(16).padStart(2, "0");
  const base = `#${channel(colour.r)}${channel(colour.g)}${channel(colour.b)}`;
  return colour.a === 255 ? base : `${base}${channel(colour.a)}`;
}

function parseHex(value: string): Pick<Rgba, "r" | "g" | "b"> | null {
  const match = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(value);
  if (!match) return null;
  return {
    r: Number.parseInt(match[1], 16),
    g: Number.parseInt(match[2], 16),
    b: Number.parseInt(match[3], 16),
  };
}
