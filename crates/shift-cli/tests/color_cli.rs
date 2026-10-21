mod support;

use serde_json::Value;
use shift_font::{Glyph, test_support::sample_font};
use shift_store::ShiftStore;
use support::{load_font, shift};

fn document_with_layers(path: &std::path::Path) {
    let mut font = sample_font();
    for name in ["heart", "heart.red", "heart.shine"] {
        font.insert_glyph(Glyph::new(name)).unwrap();
    }
    drop(ShiftStore::create_document(path, &font).unwrap());
}

#[test]
fn palettes_and_colour_glyphs_are_authored_inspected_and_removed() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("Colour.shift");
    document_with_layers(&path);
    let path_text = path.to_str().unwrap();

    let output = shift(&[
        "color",
        "palette",
        "set",
        path_text,
        "--palette",
        "Light:#FF0000,#FFFFFF80",
        "--palette",
        "Dark:#800000,#00000080",
        "--json",
    ]);
    assert!(output.status.success(), "{:?}", output.stderr);
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["changes"][0]["palettes"], 2);
    assert_eq!(report["changes"][0]["paletteColors"], 2);

    let output = shift(&[
        "color",
        "glyph",
        "set",
        path_text,
        "heart",
        "--layer",
        "heart.red=palette:0",
        "--layer",
        "heart.shine=linear:0,0,0,700:0=palette:1/1=palette:1@0",
        "--json",
    ]);
    assert!(output.status.success(), "{:?}", output.stderr);

    let font = load_font(path_text);
    assert_eq!(font.color().palettes.len(), 2);
    assert_eq!(font.color().glyphs.len(), 1);

    let output = shift(&["inspect", path_text, "--json"]);
    assert!(output.status.success(), "{:?}", output.stderr);
    let inspected: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(inspected["color"]["palettes"][0]["colors"][0], "#FF0000FF");
    assert_eq!(inspected["color"]["glyphs"][0]["name"], "heart");
    assert_eq!(
        inspected["color"]["glyphs"][0]["layers"][1]["fill"],
        "linear gradient, 2 stops"
    );

    let output = shift(&["inspect", path_text, "--view", "color"]);
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(
        text.contains("heart.red") && text.contains("palette:0"),
        "{text}"
    );

    let output = shift(&["color", "glyph", "remove", path_text, "heart"]);
    assert!(output.status.success(), "{:?}", output.stderr);
    assert!(load_font(path_text).color().glyphs.is_empty());
}

#[test]
fn invalid_colour_edits_leave_the_document_unchanged() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("Colour.shift");
    document_with_layers(&path);
    let path_text = path.to_str().unwrap();
    let original = load_font(path_text);

    let missing_entry = shift(&[
        "color",
        "glyph",
        "set",
        path_text,
        "heart",
        "--layer",
        "heart.red=palette:0",
    ]);
    assert!(!missing_entry.status.success());

    let unequal = shift(&[
        "color",
        "palette",
        "set",
        path_text,
        "--palette",
        "#FF0000,#00FF00",
        "--palette",
        "#0000FF",
    ]);
    assert!(!unequal.status.success());

    let single_palette = shift(&["color", "palette", "set", path_text, "--palette", "#FF0000"]);
    assert!(single_palette.status.success());
    let unknown = shift(&[
        "color",
        "glyph",
        "set",
        path_text,
        "heart",
        "--layer",
        "ghost=palette:0",
    ]);
    assert!(!unknown.status.success());

    let after = load_font(path_text);
    assert!(after.color().glyphs.is_empty());
    assert_eq!(after.glyph_count(), original.glyph_count());
}
