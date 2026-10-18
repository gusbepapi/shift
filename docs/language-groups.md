# Custom language groups

Shift lists the scripts and groups of the Glyphs Languages sidebar in the glyph catalogue. A font tracks a few of them by default (Latin Basic and Western European, Cyrillic Basic, Greek Basic); the **+** button beside «Languages» adds any other script, and unticking a script hides it.

Groups of your own, such as a constructed script or a private-use block, live in the `language-groups/` folder of Shift’s config folder (`~/.config/shift` on Linux and macOS, with `shift-nightly` and `shift-dev` for those builds, and the `config` folder inside the user data folder on Windows). Shift watches the folder, so a saved file applies at once to every window. Each file is JSON or a Glyphs `Groups.plist`; the extension decides which.

## JSON

```json
{
  "scripts": [
    {
      "name": "Klingon (pIqaD)",
      "groups": [
        { "name": "Letters", "ranges": [["U+F8D0", "U+F8E9"]] },
        { "name": "Digits", "codepoints": ["U+F8F0", "U+F8F1"] },
        { "name": "Marks", "section": "Extras", "codepoints": [63743] }
      ]
    }
  ]
}
```

A codepoint is a number or a string such as `U+F8D0`, `0xF8D0`, `uniF8D0`, or `u1F600`. A group may combine `codepoints` and `ranges`; `section` puts it under a heading inside its script. A group with the same identifier as a built-in group replaces it.

## Glyphs `Groups.plist`

Copy the file from Glyphs’ `Info` folder into `language-groups/`. Shift reads the `languages` entries: a group may list glyph names (`list`) or give a `predicate` of `unicodeChar` comparisons joined by `AND` and `OR`. Other predicates and the file’s `categories` are skipped.

A problem in a file skips only the entry that caused it; Shift reports it in the developer console as `language-groups/<file>: <message>`.
