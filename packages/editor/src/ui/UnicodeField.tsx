import { Input, cn } from "@shift/ui";
import { useEffect, useRef, useState } from "react";
import type { GlyphId, Unicode } from "@shift/types";
import { codepointToHex, formatCodepointAsUPlus, parseCodepointInput } from "../lib/utils/unicode";

export interface UnicodeFieldProps {
  readonly glyphId: GlyphId;
  /** Every Unicode value the glyph encodes; the first one is the one shown and edited. */
  readonly unicodes: readonly Unicode[];
  /** Whether another glyph of the font already encodes this value. */
  readonly isTaken: (codepoint: Unicode) => boolean;
  readonly editable: boolean;
  /** Replaces the glyph's Unicode values, keeping its name. */
  readonly onCommit: (unicodes: Unicode[]) => void;
}

/**
 * Shows the Unicode value of a glyph and lets a person assign or remove it.
 *
 * @remarks
 * A glyph can be drawn without any Unicode value, as in Glyphs, and receive one later.
 * Enter or leaving the field commits; Escape cancels. Blank input removes the first value.
 * Text that is not a free scalar value is discarded and the previous value returns.
 */
export function UnicodeField({
  glyphId,
  unicodes,
  isTaken,
  editable,
  onCommit,
}: UnicodeFieldProps) {
  const current = unicodes[0] ?? null;
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  const [rejected, setRejected] = useState(false);
  const cancelled = useRef(false);

  // A different glyph or an outside change ends any edit in progress
  useEffect(() => {
    setEditing(false);
    setRejected(false);
  }, [glyphId, current]);

  const label = current === null ? "—" : formatCodepointAsUPlus(current);

  if (!editable) {
    return (
      <div className={cn("font-mono text-sm", current === null && "text-secondary")}>{label}</div>
    );
  }

  const begin = (): void => {
    cancelled.current = false;
    setDraft(current === null ? "" : codepointToHex(current));
    setRejected(false);
    setEditing(true);
  };

  const commit = (): void => {
    if (cancelled.current) {
      setEditing(false);
      return;
    }

    const input = parseCodepointInput(draft);
    if (
      input.kind === "invalid" ||
      (input.kind === "codepoint" && input.codepoint !== current && isTaken(input.codepoint))
    ) {
      setRejected(true);
      setEditing(false);
      return;
    }

    setEditing(false);
    const rest = unicodes.slice(1);
    const next = input.kind === "codepoint" ? [input.codepoint, ...rest] : rest;
    const unchanged =
      next.length === unicodes.length && next.every((value, index) => value === unicodes[index]);
    if (!unchanged) onCommit(next);
  };

  if (!editing) {
    return (
      <button
        type="button"
        aria-label="Unicode value"
        title={
          rejected ? "That value is not a free Unicode scalar" : "Click to set the Unicode value"
        }
        aria-invalid={rejected || undefined}
        className={cn(
          "rounded px-1 font-mono text-sm hover:bg-surface-hover",
          current === null && "text-secondary",
          rejected && "text-destructive",
        )}
        onClick={begin}
      >
        {label}
      </button>
    );
  }

  return (
    <Input
      autoFocus
      aria-label="Unicode value"
      placeholder="0041"
      value={draft}
      size="sm"
      className="w-24 text-center font-mono"
      onChange={(event) => setDraft(event.currentTarget.value)}
      onFocus={(event) => event.currentTarget.select()}
      onBlur={commit}
      onKeyDown={(event) => {
        event.nativeEvent.stopImmediatePropagation();
        if (event.key === "Enter") event.currentTarget.blur();
        if (event.key === "Escape") {
          cancelled.current = true;
          event.currentTarget.blur();
        }
      }}
    />
  );
}
