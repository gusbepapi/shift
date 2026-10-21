import { useState } from "react";
import { useSignalState } from "@shift/editor/signals";
import { colorLayersOf, cssColor, resolvePaintColor, type ColorLayer } from "@shift/editor";
import type { PaintColor, Rgba } from "@shift/types";
import { Toggle, ToggleGroup } from "@shift/ui";
import { useEditor } from "@/workspace/WorkspaceContext";
import { SidebarSection, SidebarSubsection } from "./SidebarSection";

/**
 * The font's colour palettes and, for the shown glyph, its colour layers.
 *
 * @remarks
 * Read-only. Renders nothing for a font without palettes, and, like the Glyph section above it,
 * lists layers only when the scene holds exactly one glyph.
 */
export const GlyphColourSection = () => {
  const editor = useEditor();
  const color = useSignalState(editor.font.colorCell);
  const scene = useSignalState(editor.scene.cell);
  const [paletteIndex, setPaletteIndex] = useState(0);

  if (color.palettes.length === 0) return null;

  const palette = color.palettes[Math.min(paletteIndex, color.palettes.length - 1)];
  const glyphNodes = scene.nodes.filter((node) => node.kind === "glyph");
  const node = glyphNodes.length === 1 ? glyphNodes[0] : undefined;
  const layers = node?.kind === "glyph" ? colorLayersOf(color, node.glyphId) : [];

  return (
    <SidebarSection title="Colour">
      {color.palettes.length > 1 ? (
        <ToggleGroup
          aria-label="Palette"
          value={[String(paletteIndex)]}
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
      ) : null}
      <SidebarSubsection title="Entries">
        <ul className="flex flex-wrap gap-1" aria-label="Palette entries">
          {palette.colors.map((entry, index) => (
            <li key={index}>
              <Swatch colour={entry} label={`Entry ${index}, ${hex(entry)}`} />
            </li>
          ))}
        </ul>
      </SidebarSubsection>
      {layers.length > 0 ? (
        <SidebarSubsection title="Layers">
          <ol className="flex flex-col gap-1" aria-label="Colour layers, bottom first">
            {layers.map((layer, index) => (
              <LayerRow key={index} layer={layer} palette={palette.colors} />
            ))}
          </ol>
        </SidebarSubsection>
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
}: {
  readonly layer: ColorLayer;
  readonly palette: readonly Rgba[];
}) => {
  const editor = useEditor();
  const name = editor.font.entryForId(layer.glyph)?.name ?? layer.glyph;
  const { fill } = layer;
  const kind = FILL_LABELS[fill.kind];
  const stops = fill.kind === "solid" ? [{ color: fill.color, alpha: fill.alpha }] : fill.stops;

  return (
    <li className="flex items-center gap-2 text-ui text-secondary">
      <span className="flex shrink-0 gap-0.5">
        {stops.map((stop, index) => (
          <StopSwatch key={index} color={stop.color} alpha={stop.alpha} palette={palette} />
        ))}
      </span>
      <span className="min-w-0 flex-1 truncate text-primary">{name}</span>
      <span className="shrink-0">{kind}</span>
    </li>
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
  if (!resolved) {
    return (
      <Swatch colour={null} label={color.kind === "foreground" ? "Text colour" : "Missing entry"} />
    );
  }
  return (
    <Swatch
      colour={resolved}
      alpha={alpha}
      label={`Entry ${color.kind === "palette" ? color.index : ""}, ${hex(resolved)}`}
    />
  );
};

/** A small colour chip; a `null` colour draws the text foreground as a hatched chip. */
const Swatch = ({
  colour,
  alpha = 1,
  label,
}: {
  readonly colour: Rgba | null;
  readonly alpha?: number;
  readonly label: string;
}) => (
  <span
    role="img"
    aria-label={label}
    title={label}
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
