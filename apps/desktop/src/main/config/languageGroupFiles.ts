import fs from "node:fs";
import path from "node:path";
import type { LanguageGroupFile } from "../../shared/config/config";

export const LANGUAGE_GROUPS_FOLDER_NAME = "language-groups";

const FORMATS: Readonly<Record<string, LanguageGroupFile["format"]>> = {
  ".json": "json",
  ".plist": "plist",
};

/**
 * Reads every `*.json` and `*.plist` file in `directory`, ordered by name.
 *
 * @remarks
 * A file that cannot be read is left out, so one unreadable file never hides the others.
 * Parsing is left to the renderer, which holds the glyph database that resolves glyph names.
 */
export function readLanguageGroupFiles(directory: string): LanguageGroupFile[] {
  let names: string[];
  try {
    names = fs.readdirSync(directory);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return [];
    throw error;
  }

  const files: LanguageGroupFile[] = [];
  for (const name of names.sort()) {
    const format = FORMATS[path.extname(name).toLowerCase()];
    if (!format) continue;

    const filePath = path.join(directory, name);
    try {
      files.push({ name, path: filePath, format, text: fs.readFileSync(filePath, "utf8") });
    } catch {
      continue;
    }
  }
  return files;
}
