/**
 * Convert a Unicode codepoint to its hex representation without prefix (e.g. for URLs).
 */
export function codepointToHex(codepoint: number): string {
  return codepoint.toString(16).padStart(4, "0").toUpperCase();
}

/**
 * Format a Unicode codepoint as U+XXXX (e.g. for display).
 */
export function formatCodepointAsUPlus(codepoint: number): string {
  return `U+${codepointToHex(codepoint)}`;
}

export function fallbackGlyphNameForUnicode(unicode: number): string {
  return `uni${codepointToHex(unicode)}`;
}

/** The outcome of reading a codepoint that a person typed. */
export type CodepointInput =
  | { readonly kind: "empty" }
  | { readonly kind: "codepoint"; readonly codepoint: number }
  | { readonly kind: "invalid" };

const MAX_CODEPOINT = 0x10ffff;
const SURROGATES = { first: 0xd800, last: 0xdfff } as const;

/**
 * Reads a hexadecimal codepoint typed as `0041`, `U+0041`, `u0041` or `0x41`.
 *
 * @returns `empty` for blank input, which removes the Unicode value, and `invalid` for text
 * that is not a scalar value: not hexadecimal, above U+10FFFF, or a surrogate.
 */
export function parseCodepointInput(text: string): CodepointInput {
  const trimmed = text.trim();
  if (trimmed === "") return { kind: "empty" };

  const digits = /^(?:u\+|u|0x)?([0-9a-f]{1,6})$/i.exec(trimmed)?.[1];
  if (digits === undefined) return { kind: "invalid" };

  const codepoint = Number.parseInt(digits, 16);
  const surrogate = codepoint >= SURROGATES.first && codepoint <= SURROGATES.last;
  return codepoint > MAX_CODEPOINT || surrogate
    ? { kind: "invalid" }
    : { kind: "codepoint", codepoint };
}
