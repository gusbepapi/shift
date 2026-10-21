use shift_font::{
    ColorData, ColorGlyph, ColorPalette, Font, FontChange, FontChangeSet, GlyphId, Paint,
    PaintColor, Replacement, Rgba, test_support::sample_font,
};
use shift_store::{SHIFT_DOCUMENT_SCHEMA_VERSION, SaveState, ShiftStore};

fn colour_data(font: &Font) -> ColorData {
    let ids: Vec<GlyphId> = font.glyphs().map(shift_font::Glyph::id).collect();
    assert!(ids.len() >= 2, "sample font needs two glyphs");
    ColorData {
        palettes: vec![
            ColorPalette {
                name: Some("Light".to_string()),
                colors: vec![Rgba::new(255, 64, 0, 255), Rgba::new(0, 0, 0, 128)],
            },
            ColorPalette {
                name: None,
                colors: vec![Rgba::new(0, 64, 255, 255), Rgba::new(255, 255, 255, 128)],
            },
        ],
        glyphs: vec![ColorGlyph {
            glyph: ids[0].clone(),
            paint: Paint::Layers {
                layers: vec![Paint::Glyph {
                    glyph: ids[1].clone(),
                    fill: Box::new(Paint::Solid {
                        color: PaintColor::Palette { index: 1 },
                        alpha: 0.75,
                    }),
                }],
            },
        }],
    }
}

fn with_colour(font: &Font) -> Font {
    let mut coloured = font.clone();
    coloured
        .set_color(colour_data(font))
        .expect("valid colour data");
    coloured
}

fn colour_change(before: &Font, after: &Font) -> FontChangeSet {
    FontChangeSet::new(vec![FontChange::Color(Replacement::new(
        before.color().clone(),
        after.color().clone(),
    ))])
}

#[test]
fn colour_data_survives_create_and_reopen() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("Colour.shift");
    let coloured = with_colour(&sample_font());
    drop(ShiftStore::create_document(&path, &coloured).unwrap());

    let reopened = ShiftStore::open_document(&path).unwrap();
    assert_eq!(reopened.load_font_state().unwrap(), coloured);
}

#[test]
fn colour_change_sets_apply_and_reverse() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("working.sqlite");
    let original = sample_font();
    let coloured = with_colour(&original);
    let mut store = ShiftStore::open(&path).unwrap();
    store.replace_font_state(&original).unwrap();

    let changes = colour_change(&original, &coloured);
    store.apply_change_set(&changes).unwrap();
    assert_eq!(store.load_font_state().unwrap(), coloured);

    store.apply_change_set(&changes.inverted()).unwrap();
    assert_eq!(store.load_font_state().unwrap(), original);
}

#[test]
fn version_three_documents_gain_empty_colour_tables_on_open() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("Old.shift");
    let original = sample_font();
    drop(ShiftStore::create_document(&path, &original).unwrap());
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "DROP TABLE color_palettes; DROP TABLE color_glyphs; PRAGMA user_version = 3;",
        )
        .unwrap();
    }

    let store = ShiftStore::open_document(&path).unwrap();
    assert_eq!(store.load_font_state().unwrap(), original);
    drop(store);

    let conn = rusqlite::Connection::open(&path).unwrap();
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, SHIFT_DOCUMENT_SCHEMA_VERSION);
}

#[test]
fn recovery_overlay_restores_colour_data() {
    let temp = tempfile::tempdir().unwrap();
    let document_path = temp.path().join("Colour.shift");
    let recovery_path = temp.path().join("Colour.recovery.sqlite");
    let original = sample_font();
    let coloured = with_colour(&original);
    drop(ShiftStore::create_document(&document_path, &original).unwrap());

    let mut document =
        ShiftStore::open_document_with_recovery(&document_path, &recovery_path).unwrap();
    document
        .apply_change_set_with_font(
            &colour_change(&original, &coloured),
            &coloured,
            SaveState::Unsaved,
        )
        .unwrap();
    drop(document);

    let canonical = ShiftStore::open_document(&document_path).unwrap();
    assert_eq!(canonical.load_font_state().unwrap(), original);
    drop(canonical);

    let mut reopened =
        ShiftStore::open_document_with_recovery(&document_path, &recovery_path).unwrap();
    assert_eq!(reopened.load_font_state().unwrap(), coloured);
    reopened.save_document().unwrap();
    drop(reopened);

    let saved = ShiftStore::open_document(&document_path).unwrap();
    assert_eq!(saved.load_font_state().unwrap(), coloured);
}
