import type { Language } from "./types.js";

/** Standard Moroccan Tamazight is standardised, not invented, though Hyperglot marks it constructed. */
const STANDARDISED_LANGUAGE_CODES: ReadonlySet<string> = new Set(["zgh"]);

/** Whether a language was invented, such as Esperanto or Klingon. */
export function isConstructedLanguage(language: Pick<Language, "id" | "status">): boolean {
  return (
    language.status === "constructed" &&
    !STANDARDISED_LANGUAGE_CODES.has(language.id.split("-", 1)[0] ?? "")
  );
}
