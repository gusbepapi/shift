import type { LanguageGroup } from "./types.js";

/** Largest Unicode scalar value. */
const MAX_CODEPOINT = 0x10ffff;

/** Maps a glyph name such as `aacute` to its codepoint, or `null` when it has none. */
export type GlyphNameResolver = (name: string) => number | null;

/** Groups read from a user's file, with a message for each entry that could not be used. */
export interface CustomGroupsResult {
  groups: LanguageGroup[];
  problems: string[];
}

type CodepointRange = readonly [number, number];

/**
 * Reads groups from Shift's JSON format.
 *
 * @remarks
 * The file holds `{ "scripts": [{ "name": "Klingon", "groups": [{ "name": "Letters",
 * "section": "optional", "codepoints": ["U+F8D0", 63697], "ranges": [["U+F8D0", "U+F8E9"]] }] }] }`.
 * A codepoint is a number or a string such as `U+F8D0`, `0xF8D0`, `uniF8D0`, or `u1F600`.
 * A problem skips the entry that caused it and keeps the rest.
 *
 * @param text - The file's contents.
 */
export function parseCustomGroups(text: string): CustomGroupsResult {
  const result: CustomGroupsResult = { groups: [], problems: [] };

  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch (error) {
    result.problems.push(`not valid JSON: ${error instanceof Error ? error.message : error}`);
    return result;
  }

  const scripts = isRecord(parsed) ? parsed["scripts"] : undefined;
  if (!Array.isArray(scripts)) {
    result.problems.push('expected an object with a "scripts" array');
    return result;
  }

  for (const script of scripts) {
    if (!isRecord(script) || typeof script["name"] !== "string" || script["name"].trim() === "") {
      result.problems.push("a script has no name");
      continue;
    }

    const scriptName = script["name"].trim();
    const groups = script["groups"];
    if (!Array.isArray(groups)) {
      result.problems.push(`${scriptName}: expected a "groups" array`);
      continue;
    }

    for (const group of groups) {
      if (!isRecord(group) || typeof group["name"] !== "string" || group["name"].trim() === "") {
        result.problems.push(`${scriptName}: a group has no name`);
        continue;
      }

      const name = group["name"].trim();
      const section = typeof group["section"] === "string" ? group["section"].trim() : "";
      const codepoints = new Set<number>();

      for (const value of asArray(group["codepoints"])) {
        const codepoint = parseCodepoint(value);
        if (codepoint === null)
          result.problems.push(`${scriptName} ${name}: bad codepoint ${show(value)}`);
        else codepoints.add(codepoint);
      }

      for (const pair of asArray(group["ranges"])) {
        const from = Array.isArray(pair) ? parseCodepoint(pair[0]) : null;
        const to = Array.isArray(pair) ? parseCodepoint(pair[1]) : null;
        if (from === null || to === null || from > to) {
          result.problems.push(`${scriptName} ${name}: bad range ${show(pair)}`);
          continue;
        }
        addRange(codepoints, [from, to]);
      }

      pushGroup(result, scriptName, section === "" ? null : section, name, codepoints);
    }
  }

  return result;
}

/**
 * Reads the `languages` entries of a Glyphs `Groups.plist`.
 *
 * @remarks
 * Glyphs writes the old-style property list that `.glyphs` files use. A group may list glyph names
 * (`list`, or `coverage` in older files) or give a `predicate` made of `unicodeChar` comparisons
 * joined by `AND` and `OR`. Other predicates, and the file's `categories`, are reported and skipped.
 *
 * @param text - The file's contents.
 * @param resolveGlyphName - Resolves names that do not spell their codepoint, such as `aacute`.
 */
export function parseGlyphsGroupsPlist(
  text: string,
  resolveGlyphName: GlyphNameResolver,
): CustomGroupsResult {
  const result: CustomGroupsResult = { groups: [], problems: [] };

  let root: PlistValue;
  try {
    root = new PlistReader(text).read();
  } catch (error) {
    result.problems.push(`not a property list: ${error instanceof Error ? error.message : error}`);
    return result;
  }

  if (!isPlistDict(root)) {
    result.problems.push("expected a dictionary at the top level");
    return result;
  }

  if (root.categories !== undefined) {
    result.problems.push("categories are Glyphs filters, not languages, and were skipped");
  }

  const languages = root.languages;
  if (!Array.isArray(languages)) {
    result.problems.push('no "languages" array');
    return result;
  }

  for (const language of languages) {
    if (!isPlistDict(language) || typeof language.name !== "string") {
      result.problems.push("a language has no name");
      continue;
    }

    readPlistEntry(result, language.name, null, language.name, language, resolveGlyphName);
  }

  return result;
}

function readPlistEntry(
  result: CustomGroupsResult,
  script: string,
  section: string | null,
  name: string,
  entry: Record<string, PlistValue>,
  resolveGlyphName: GlyphNameResolver,
): void {
  const children = entry.subGroup;
  if (Array.isArray(children)) {
    for (const child of children) {
      if (!isPlistDict(child) || typeof child.name !== "string") {
        result.problems.push(`${script}: a subgroup has no name`);
        continue;
      }

      const nested = Array.isArray(child.subGroup);
      readPlistEntry(
        result,
        script,
        nested ? child.name : section,
        child.name,
        child,
        resolveGlyphName,
      );
    }
    return;
  }

  const codepoints = new Set<number>();
  const names = entry.list ?? entry.coverage;

  if (Array.isArray(names)) {
    const unresolved: string[] = [];
    for (const glyphName of names) {
      if (typeof glyphName !== "string") continue;

      const codepoint = codepointFromGlyphName(glyphName) ?? resolveGlyphName(glyphName);
      if (codepoint === null) unresolved.push(glyphName);
      else codepoints.add(codepoint);
    }
    if (unresolved.length > 0) {
      const shown = unresolved.slice(0, 5).join(", ");
      const rest = unresolved.length > 5 ? ` and ${unresolved.length - 5} more` : "";
      result.problems.push(`${script} ${name}: no codepoint for ${shown}${rest}`);
    }
  }

  if (typeof entry.predicate === "string") {
    try {
      for (const range of parseCodepointPredicate(entry.predicate)) addRange(codepoints, range);
    } catch (error) {
      result.problems.push(`${script} ${name}: ${error instanceof Error ? error.message : error}`);
    }
  }

  pushGroup(result, script, section, name, codepoints);
}

function pushGroup(
  result: CustomGroupsResult,
  script: string,
  section: string | null,
  name: string,
  codepoints: ReadonlySet<number>,
): void {
  if (codepoints.size === 0) {
    result.problems.push(`${script} ${name}: no codepoints`);
    return;
  }

  const parts = [script, section ?? "", name].filter((part) => part !== "");
  const baseId = `custom-${parts.map(slugify).join("-")}`;
  let id = baseId;
  for (let suffix = 2; result.groups.some((group) => group.id === id); suffix += 1) {
    id = `${baseId}-${suffix}`;
  }

  result.groups.push({
    id,
    name,
    script,
    section,
    baseCodepoints: [...codepoints].sort((left, right) => left - right),
    basis: "custom",
  });
}

/**
 * Parses a Glyphs predicate made of `unicodeChar` comparisons into the ranges it accepts.
 *
 * @throws When the predicate uses anything but `unicodeChar`, numbers, `AND`, `OR`, and parentheses.
 */
export function parseCodepointPredicate(predicate: string): CodepointRange[] {
  const tokens = predicate.match(
    /\(|\)|>=|<=|==|=|>|<|[A-Za-z_][A-Za-z0-9_]*|0[xX][0-9A-Fa-f]+|\d+|\S/g,
  );
  if (!tokens) throw new Error("empty predicate");

  let position = 0;
  const peek = (): string | undefined => tokens[position];
  const take = (): string => {
    const token = tokens[position];
    if (token === undefined) throw new Error(`predicate ends early: ${predicate}`);
    position += 1;
    return token;
  };

  const parseOr = (): CodepointRange[] => {
    let ranges = parseAnd();
    while (peek()?.toUpperCase() === "OR") {
      take();
      ranges = unionRanges(ranges, parseAnd());
    }
    return ranges;
  };
  const parseAnd = (): CodepointRange[] => {
    let ranges = parseFactor();
    while (peek()?.toUpperCase() === "AND") {
      take();
      ranges = intersectRanges(ranges, parseFactor());
    }
    return ranges;
  };
  const parseFactor = (): CodepointRange[] => {
    if (peek() === "(") {
      take();
      const inner = parseOr();
      if (take() !== ")") throw new Error(`missing parenthesis: ${predicate}`);
      return inner;
    }

    const subject = take();
    if (subject !== "unicodeChar") throw new Error(`unsupported predicate on ${subject}`);

    const operator = take();
    const value = Number(take());
    if (!Number.isInteger(value) || value < 0 || value > MAX_CODEPOINT) {
      throw new Error(`unsupported value in ${predicate}`);
    }

    switch (operator) {
      case ">=":
        return [[value, MAX_CODEPOINT]];
      case ">":
        return value === MAX_CODEPOINT ? [] : [[value + 1, MAX_CODEPOINT]];
      case "<=":
        return [[0, value]];
      case "<":
        return value === 0 ? [] : [[0, value - 1]];
      case "==":
      case "=":
        return [[value, value]];
      default:
        throw new Error(`unsupported operator ${operator}`);
    }
  };

  const ranges = parseOr();
  if (position < tokens.length) throw new Error(`unexpected ${tokens[position]} in ${predicate}`);
  return ranges;
}

function unionRanges(left: CodepointRange[], right: CodepointRange[]): CodepointRange[] {
  const sorted = [...left, ...right].sort((a, b) => a[0] - b[0]);
  const merged: [number, number][] = [];
  for (const [from, to] of sorted) {
    const last = merged[merged.length - 1];
    if (last && from <= last[1] + 1) last[1] = Math.max(last[1], to);
    else merged.push([from, to]);
  }
  return merged;
}

function intersectRanges(left: CodepointRange[], right: CodepointRange[]): CodepointRange[] {
  const intersection: CodepointRange[] = [];
  for (const [leftFrom, leftTo] of left) {
    for (const [rightFrom, rightTo] of right) {
      const from = Math.max(leftFrom, rightFrom);
      const to = Math.min(leftTo, rightTo);
      if (from <= to) intersection.push([from, to]);
    }
  }
  return unionRanges(intersection, []);
}

function addRange(codepoints: Set<number>, [from, to]: CodepointRange): void {
  for (let codepoint = from; codepoint <= to; codepoint += 1) codepoints.add(codepoint);
}

/** Reads `uniF8D0`, `u12000`, and `U+F8D0` style names, which spell their codepoint. */
export function codepointFromGlyphName(name: string): number | null {
  const match = /^(?:uni|u|U\+|0x)([0-9A-Fa-f]{4,6})$/.exec(name);
  if (!match?.[1]) return null;

  const codepoint = Number.parseInt(match[1], 16);
  return codepoint <= MAX_CODEPOINT ? codepoint : null;
}

function parseCodepoint(value: unknown): number | null {
  if (typeof value === "number") {
    return Number.isInteger(value) && value >= 0 && value <= MAX_CODEPOINT ? value : null;
  }
  if (typeof value !== "string") return null;

  return codepointFromGlyphName(value.trim());
}

type PlistValue = string | PlistValue[] | { [key: string]: PlistValue };

function isPlistDict(value: PlistValue | undefined): value is { [key: string]: PlistValue } {
  return typeof value === "object" && !Array.isArray(value);
}

/** Reads the old-style property list: dictionaries, arrays, and strings, with comments. */
class PlistReader {
  readonly #text: string;
  #position = 0;

  constructor(text: string) {
    this.#text = text;
  }

  read(): PlistValue {
    const value = this.#value();
    this.#skipSpace();
    if (this.#position < this.#text.length) throw this.#error("unexpected text after the value");
    return value;
  }

  #value(): PlistValue {
    this.#skipSpace();
    const character = this.#text[this.#position];

    switch (character) {
      case "{":
        return this.#dict();
      case "(":
        return this.#array();
      case '"':
        return this.#quoted();
      case undefined:
        throw this.#error("the file ends early");
      default:
        return this.#bare();
    }
  }

  #dict(): PlistValue {
    this.#position += 1;
    const dict: { [key: string]: PlistValue } = {};

    for (;;) {
      this.#skipSpace();
      if (this.#text[this.#position] === "}") {
        this.#position += 1;
        return dict;
      }

      const key = this.#value();
      if (typeof key !== "string") throw this.#error("a key must be a string");

      this.#skipSpace();
      this.#expect("=");
      dict[key] = this.#value();
      this.#skipSpace();
      this.#expect(";");
    }
  }

  #array(): PlistValue {
    this.#position += 1;
    const items: PlistValue[] = [];

    for (;;) {
      this.#skipSpace();
      const character = this.#text[this.#position];
      if (character === ")") {
        this.#position += 1;
        return items;
      }
      if (character === ",") {
        this.#position += 1;
        continue;
      }

      items.push(this.#value());
    }
  }

  #quoted(): string {
    this.#position += 1;
    let value = "";

    for (;;) {
      const character = this.#text[this.#position];
      if (character === undefined) throw this.#error("a string is not closed");
      this.#position += 1;

      if (character === '"') return value;
      if (character !== "\\") {
        value += character;
        continue;
      }

      const escaped = this.#text[this.#position];
      this.#position += 1;
      switch (escaped) {
        case "n":
          value += "\n";
          break;
        case "t":
          value += "\t";
          break;
        case "U": {
          const hex = this.#text.slice(this.#position, this.#position + 4);
          this.#position += 4;
          value += String.fromCodePoint(Number.parseInt(hex, 16));
          break;
        }
        default:
          value += escaped ?? "";
      }
    }
  }

  #bare(): string {
    const start = this.#position;
    while (
      this.#position < this.#text.length &&
      /[A-Za-z0-9_.$/:@+\-]/.test(this.#text[this.#position] ?? "")
    ) {
      this.#position += 1;
    }
    if (this.#position === start) throw this.#error(`unexpected ${this.#text[this.#position]}`);

    return this.#text.slice(start, this.#position);
  }

  #expect(character: string): void {
    if (this.#text[this.#position] !== character) throw this.#error(`expected ${character}`);
    this.#position += 1;
  }

  #skipSpace(): void {
    for (;;) {
      const character = this.#text[this.#position];
      if (character !== undefined && /\s/.test(character)) {
        this.#position += 1;
        continue;
      }

      const pair = this.#text.slice(this.#position, this.#position + 2);
      if (pair === "//") {
        const end = this.#text.indexOf("\n", this.#position);
        this.#position = end === -1 ? this.#text.length : end + 1;
        continue;
      }
      if (pair === "/*") {
        const end = this.#text.indexOf("*/", this.#position + 2);
        this.#position = end === -1 ? this.#text.length : end + 2;
        continue;
      }
      return;
    }
  }

  #error(message: string): Error {
    const line = this.#text.slice(0, this.#position).split("\n").length;
    return new Error(`${message} (line ${line})`);
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function asArray(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
}

function show(value: unknown): string {
  return typeof value === "string" ? `"${value}"` : JSON.stringify(value);
}

function slugify(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/(^-|-$)/g, "");
}
