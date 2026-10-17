import glyphData from "../resources/glyph-data.json";
import charsets from "../resources/charsets.json";
import languages from "../resources/languages.json";
import type { GlyphInfoCoreResources, Language } from "./types.js";

export const coreResources: GlyphInfoCoreResources = {
  glyphData,
  charsets,
  // The JSON import widens the status fields to `string`, so narrowing them here
  languages: languages.languages as Language[],
};
