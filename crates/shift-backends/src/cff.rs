//! Converts a compiled TrueType binary into an OpenType binary with CFF outlines.
//!
//! fontc compiles TrueType (`glyf`) outlines only. This module reads those outlines back,
//! raises every quadratic segment to the equivalent cubic one, encodes the result as Type 2
//! charstrings in a `CFF ` table, and swaps that table in for `glyf` and `loca`. Layout,
//! metrics, names and every other table carry over unchanged.
//!
//! Only static fonts are converted. A variable font needs `CFF2`, which this module does
//! not write.

use crate::numeric::{clamped_i16, clamped_u8, lossy_f32, rounded_i32};
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::string::StringId;
use skrifa::{FontRef as SkrifaFont, GlyphId, MetadataProvider};
use write_fonts::FontBuilder;
use write_fonts::from_obj::ToOwnedTable;
use write_fonts::read::{FontRef, TableProvider};
use write_fonts::tables::maxp::Maxp;
use write_fonts::tables::post::Post;
use write_fonts::types::{Tag, Version16Dot16};

/// Describes a failure to build an OpenType binary with CFF outlines.
#[derive(Debug, thiserror::Error)]
pub enum CffError {
    #[error("cannot read the compiled font: {message}")]
    ReadFont { message: String },

    #[error("variable fonts need CFF2 outlines, which this export does not write")]
    VariableFont,

    #[error("cannot draw glyph {glyph}: {message}")]
    DrawGlyph { glyph: u32, message: String },

    #[error("the font is too large for a CFF table: {message}")]
    TooLarge { message: String },

    #[error("cannot rebuild the font tables: {message}")]
    Rebuild { message: String },
}

fn read_error(error: &impl std::fmt::Display) -> CffError {
    CffError::ReadFont {
        message: error.to_string(),
    }
}

fn rebuild_error(error: &impl std::fmt::Display) -> CffError {
    CffError::Rebuild {
        message: error.to_string(),
    }
}

/// Tables that only make sense next to `glyf` outlines.
const TRUETYPE_ONLY: [&[u8; 4]; 5] = [b"glyf", b"loca", b"cvt ", b"fpgm", b"prep"];

// Coordinates are kept as multiples of 1/64, which 16.16 fixed point holds exactly
const COORDINATE_STEPS: f64 = 64.0;

// Charstring operators
const OP_RLINETO: u8 = 5;
const OP_RRCURVETO: u8 = 8;
const OP_ENDCHAR: u8 = 14;
const OP_RMOVETO: u8 = 21;

/// Converts a compiled static TrueType binary into an OpenType binary with a `CFF ` table.
///
/// # Errors
///
/// Returns [`CffError`] when the font is variable, cannot be read, or cannot be rebuilt.
pub(crate) fn ttf_to_otf(ttf: &[u8]) -> Result<Vec<u8>, CffError> {
    let font = FontRef::new(ttf).map_err(|error| read_error(&error))?;
    if font.table_data(Tag::new(b"fvar")).is_some() {
        return Err(CffError::VariableFont);
    }
    let drawn = SkrifaFont::new(ttf).map_err(|error| read_error(&error))?;

    let cff = build_cff(&font, &drawn)?;
    let num_glyphs = font
        .maxp()
        .map_err(|error| read_error(&error))?
        .num_glyphs();

    let maxp = Maxp {
        num_glyphs,
        ..Default::default()
    };
    let mut post: Post = font
        .post()
        .map_err(|error| read_error(&error))?
        .to_owned_table();
    // CFF carries the glyph names, so the post table drops its own list
    post.version = Version16Dot16::VERSION_3_0;
    post.num_glyphs = None;
    post.glyph_name_index = None;
    post.string_data = None;

    let mut builder = FontBuilder::new();
    builder
        .add_table(&maxp)
        .map_err(|error| rebuild_error(&error))?;
    builder
        .add_table(&post)
        .map_err(|error| rebuild_error(&error))?;
    builder.add_raw(Tag::new(b"CFF "), cff);
    for record in font.table_directory.table_records() {
        let tag = record.tag();
        let dropped = TRUETYPE_ONLY.iter().any(|only| tag == Tag::new(only));
        if dropped || tag == Tag::new(b"maxp") || tag == Tag::new(b"post") {
            continue;
        }
        if let Some(data) = font.table_data(tag) {
            builder.add_raw(tag, data.as_bytes().to_vec());
        }
    }
    Ok(builder.build())
}

fn build_cff(font: &FontRef<'_>, drawn: &SkrifaFont<'_>) -> Result<Vec<u8>, CffError> {
    let num_glyphs = font
        .maxp()
        .map_err(|error| read_error(&error))?
        .num_glyphs();
    let head = font.head().map_err(|error| read_error(&error))?;
    let units_per_em = head.units_per_em();

    let outlines = drawn.outline_glyphs();
    let metrics = drawn.glyph_metrics(Size::unscaled(), LocationRef::default());
    let names = drawn.glyph_names();

    let mut charstrings = Vec::with_capacity(usize::from(num_glyphs));
    let mut glyph_names = Vec::new();
    for index in 0..u32::from(num_glyphs) {
        let id = GlyphId::new(index);
        let mut pen = CharstringPen::default();
        if let Some(outline) = outlines.get(id) {
            outline
                .draw(
                    DrawSettings::unhinted(Size::unscaled(), LocationRef::default()),
                    &mut pen,
                )
                .map_err(|error| CffError::DrawGlyph {
                    glyph: index,
                    message: error.to_string(),
                })?;
        }
        let advance = metrics.advance_width(id).unwrap_or(0.0);
        charstrings.push(pen.finish(advance));

        if index > 0 {
            let name = names
                .get(id)
                .map_or_else(|| format!("glyph{index}"), |name| name.to_string());
            glyph_names.push(name);
        }
    }

    let postscript_name = drawn
        .localized_strings(StringId::POSTSCRIPT_NAME)
        .english_or_first()
        .map_or_else(|| "Untitled".to_string(), |name| name.chars().collect());

    assemble(&Assembly {
        postscript_name: &postscript_name,
        glyph_names: &glyph_names,
        charstrings: &charstrings,
        units_per_em,
        bounds: [head.x_min(), head.y_min(), head.x_max(), head.y_max()],
    })
}

struct Assembly<'a> {
    postscript_name: &'a str,
    /// Names of every glyph except `.notdef`, which CFF names implicitly.
    glyph_names: &'a [String],
    charstrings: &'a [Vec<u8>],
    units_per_em: u16,
    bounds: [i16; 4],
}

/// Standard strings occupy the first 391 string identifiers; custom ones follow.
const FIRST_CUSTOM_SID: usize = 391;

const OP_FONT_BBOX: u8 = 5;
const OP_CHARSET: u8 = 15;
const OP_CHARSTRINGS: u8 = 17;
const OP_PRIVATE: u8 = 18;
const OP_FONT_MATRIX: [u8; 2] = [12, 7];
const OP_DEFAULT_WIDTH_X: u8 = 20;
const OP_NOMINAL_WIDTH_X: u8 = 21;

fn assemble(assembly: &Assembly<'_>) -> Result<Vec<u8>, CffError> {
    let name_index = index(&[assembly.postscript_name.as_bytes().to_vec()])?;
    let strings: Vec<Vec<u8>> = assembly
        .glyph_names
        .iter()
        .map(|name| name.as_bytes().to_vec())
        .collect();
    let string_index = index(&strings)?;
    let global_subrs = index(&[])?;
    let charstrings = index(assembly.charstrings)?;

    // Both widths are zero, so every charstring states its advance width
    let mut private = Vec::new();
    private.extend(dict_int(0));
    private.push(OP_DEFAULT_WIDTH_X);
    private.extend(dict_int(0));
    private.push(OP_NOMINAL_WIDTH_X);

    let mut charset = vec![0u8];
    for number in 0..assembly.glyph_names.len() {
        let sid = u16::try_from(FIRST_CUSTOM_SID + number).map_err(|_| CffError::TooLarge {
            message: "more than 65 000 glyphs".to_string(),
        })?;
        charset.extend(sid.to_be_bytes());
    }

    // Offsets use a fixed five-byte form, so the dictionary has the same size on both passes
    let top_dict = |charset_at: usize, charstrings_at: usize, private_at: usize| {
        let mut dict = Vec::new();
        for bound in assembly.bounds {
            dict.extend(dict_int(i32::from(bound)));
        }
        dict.push(OP_FONT_BBOX);
        if assembly.units_per_em != 1000 {
            let scale = 1.0 / f64::from(assembly.units_per_em);
            for value in [scale, 0.0, 0.0, scale, 0.0, 0.0] {
                dict.extend(dict_real(value));
            }
            dict.extend(OP_FONT_MATRIX);
        }
        dict.extend(dict_offset(charset_at));
        dict.push(OP_CHARSET);
        dict.extend(dict_offset(charstrings_at));
        dict.push(OP_CHARSTRINGS);
        dict.extend(dict_offset(private.len()));
        dict.extend(dict_offset(private_at));
        dict.push(OP_PRIVATE);
        dict
    };

    let header_len = 4;
    let top_len = index(&[top_dict(0, 0, 0)])?.len();
    let charset_at =
        header_len + name_index.len() + top_len + string_index.len() + global_subrs.len();
    let charstrings_at = charset_at + charset.len();
    let private_at = charstrings_at + charstrings.len();

    let mut out = vec![1, 0, 4, 4];
    out.extend(name_index);
    out.extend(index(&[top_dict(charset_at, charstrings_at, private_at)])?);
    out.extend(string_index);
    out.extend(global_subrs);
    out.extend(charset);
    out.extend(charstrings);
    out.extend(private);
    Ok(out)
}

/// Encodes a CFF INDEX: a count, an offset size, `count + 1` offsets, then the data.
fn index(items: &[Vec<u8>]) -> Result<Vec<u8>, CffError> {
    let count = u16::try_from(items.len()).map_err(|_| CffError::TooLarge {
        message: "more than 65 535 entries in one INDEX".to_string(),
    })?;
    let mut out = count.to_be_bytes().to_vec();
    if items.is_empty() {
        return Ok(out);
    }

    let total: usize = items.iter().map(Vec::len).sum::<usize>() + 1;
    let offset_size: u8 = match total {
        0..=0xff => 1,
        0x100..=0xffff => 2,
        0x1_0000..=0xff_ffff => 3,
        _ => 4,
    };
    out.push(offset_size);

    let mut offset = 1usize;
    let push_offset = |out: &mut Vec<u8>, value: usize| {
        let bytes = u32::try_from(value).unwrap_or(u32::MAX).to_be_bytes();
        out.extend_from_slice(&bytes[4 - usize::from(offset_size)..]);
    };
    push_offset(&mut out, offset);
    for item in items {
        offset += item.len();
        push_offset(&mut out, offset);
    }
    for item in items {
        out.extend_from_slice(item);
    }
    Ok(out)
}

/// A DICT integer in the shortest form.
fn dict_int(value: i32) -> Vec<u8> {
    match value {
        -107..=107 => vec![clamped_u8(value + 139)],
        108..=1131 => {
            let value = value - 108;
            vec![clamped_u8(value / 256 + 247), clamped_u8(value % 256)]
        }
        -1131..=-108 => {
            let value = -value - 108;
            vec![clamped_u8(value / 256 + 251), clamped_u8(value % 256)]
        }
        -32768..=32767 => {
            let bytes = clamped_i16(value).to_be_bytes();
            vec![28, bytes[0], bytes[1]]
        }
        _ => {
            let mut out = vec![29];
            out.extend(value.to_be_bytes());
            out
        }
    }
}

/// A DICT integer that always takes five bytes, for offsets that are not yet known.
fn dict_offset(value: usize) -> Vec<u8> {
    let mut out = vec![29];
    out.extend(u32::try_from(value).unwrap_or(u32::MAX).to_be_bytes());
    out
}

/// A DICT real number, written as packed decimal nibbles.
fn dict_real(value: f64) -> Vec<u8> {
    let text = format!("{value:.12}");
    let text = text.trim_end_matches('0');
    let text = text.strip_suffix('.').unwrap_or(text);
    let mut nibbles: Vec<u8> = text
        .chars()
        .map(|character| match character {
            '.' => 0xa,
            '-' => 0xe,
            digit => digit
                .to_digit(10)
                .and_then(|digit| u8::try_from(digit).ok())
                .unwrap_or(0),
        })
        .collect();
    nibbles.push(0xf);
    if nibbles.len() % 2 == 1 {
        nibbles.push(0xf);
    }

    let mut out = vec![30];
    out.extend(nibbles.chunks(2).map(|pair| (pair[0] << 4) | pair[1]));
    out
}

/// Encodes a Type 2 charstring number.
fn charstring_number(value: f64) -> Vec<u8> {
    let whole = value.round();
    if (value - whole).abs() < f64::EPSILON {
        let whole = rounded_i32(whole);
        return match whole {
            -107..=107 => vec![clamped_u8(whole + 139)],
            108..=1131 => {
                let value = whole - 108;
                vec![clamped_u8(value / 256 + 247), clamped_u8(value % 256)]
            }
            -1131..=-108 => {
                let value = -whole - 108;
                vec![clamped_u8(value / 256 + 251), clamped_u8(value % 256)]
            }
            _ => {
                let bytes = clamped_i16(whole).to_be_bytes();
                vec![28, bytes[0], bytes[1]]
            }
        };
    }

    // Anything fractional is written as a 16.16 fixed point number
    let mut out = vec![255];
    out.extend(rounded_i32(value * 65536.0).to_be_bytes());
    out
}

fn quantise(value: f32) -> f64 {
    (f64::from(value) * COORDINATE_STEPS).round() / COORDINATE_STEPS
}

/// Records one glyph outline as the byte sequence of a Type 2 charstring.
#[derive(Default)]
struct CharstringPen {
    body: Vec<u8>,
    current: (f64, f64),
}

impl CharstringPen {
    fn operator(&mut self, arguments: &[f64], operator: u8) {
        for argument in arguments {
            self.body.extend(charstring_number(*argument));
        }
        self.body.push(operator);
    }

    fn finish(mut self, advance: f32) -> Vec<u8> {
        let mut out = Vec::new();
        // The width precedes the first stack-clearing operator
        if advance != 0.0 {
            out.extend(charstring_number(f64::from(advance).round()));
        }
        out.append(&mut self.body);
        out.push(OP_ENDCHAR);
        out
    }
}

impl OutlinePen for CharstringPen {
    fn move_to(&mut self, x: f32, y: f32) {
        let target = (quantise(x), quantise(y));
        self.operator(
            &[target.0 - self.current.0, target.1 - self.current.1],
            OP_RMOVETO,
        );
        self.current = target;
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let target = (quantise(x), quantise(y));
        self.operator(
            &[target.0 - self.current.0, target.1 - self.current.1],
            OP_RLINETO,
        );
        self.current = target;
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        // A quadratic segment equals the cubic whose controls sit two thirds of the way
        // from each end to the quadratic control point
        let control = (f64::from(cx0), f64::from(cy0));
        let end = (f64::from(x), f64::from(y));
        let from = self.current;
        let first = (
            from.0 + 2.0 / 3.0 * (control.0 - from.0),
            from.1 + 2.0 / 3.0 * (control.1 - from.1),
        );
        let second = (
            end.0 + 2.0 / 3.0 * (control.0 - end.0),
            end.1 + 2.0 / 3.0 * (control.1 - end.1),
        );
        self.curve_to(
            lossy_f32(first.0),
            lossy_f32(first.1),
            lossy_f32(second.0),
            lossy_f32(second.1),
            x,
            y,
        );
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let first = (quantise(cx0), quantise(cy0));
        let second = (quantise(cx1), quantise(cy1));
        let end = (quantise(x), quantise(y));
        self.operator(
            &[
                first.0 - self.current.0,
                first.1 - self.current.1,
                second.0 - first.0,
                second.1 - first.1,
                end.0 - second.0,
                end.1 - second.1,
            ],
            OP_RRCURVETO,
        );
        self.current = end;
    }

    fn close(&mut self) {
        // A Type 2 subpath closes itself, and the next move stays relative to the last point drawn
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FontExporter;
    use kurbo::{BezPath, Shape};
    use shift_font::test_support::sample_variable_font;

    /// Gathers a glyph outline as a path of the font it is drawn from.
    #[derive(Default)]
    struct PathPen(BezPath);

    impl OutlinePen for PathPen {
        fn move_to(&mut self, x: f32, y: f32) {
            self.0.move_to((f64::from(x), f64::from(y)));
        }
        fn line_to(&mut self, x: f32, y: f32) {
            self.0.line_to((f64::from(x), f64::from(y)));
        }
        fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
            self.0.quad_to(
                (f64::from(cx0), f64::from(cy0)),
                (f64::from(x), f64::from(y)),
            );
        }
        fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
            self.0.curve_to(
                (f64::from(cx0), f64::from(cy0)),
                (f64::from(cx1), f64::from(cy1)),
                (f64::from(x), f64::from(y)),
            );
        }
        fn close(&mut self) {
            self.0.close_path();
        }
    }

    fn outline_of(font: &SkrifaFont<'_>, id: GlyphId) -> BezPath {
        let mut pen = PathPen::default();
        font.outline_glyphs()
            .get(id)
            .unwrap()
            .draw(
                DrawSettings::unhinted(Size::unscaled(), LocationRef::default()),
                &mut pen,
            )
            .unwrap();
        pen.0
    }

    /// The default master of the sample font, without its variation tables.
    fn static_ttf() -> Vec<u8> {
        let variable = FontExporter::new()
            .compile_ttf(&sample_variable_font())
            .unwrap();
        let font = FontRef::new(&variable).unwrap();
        let variation_tables: [&[u8; 4]; 7] = [
            b"fvar", b"gvar", b"avar", b"HVAR", b"MVAR", b"STAT", b"cvar",
        ];
        let mut builder = FontBuilder::new();
        for record in font.table_directory.table_records() {
            let tag = record.tag();
            if variation_tables
                .iter()
                .any(|variation| tag == Tag::new(variation))
            {
                continue;
            }
            builder.add_raw(tag, font.table_data(tag).unwrap().as_bytes().to_vec());
        }
        builder.build()
    }

    #[test]
    fn colour_tables_are_copied_into_the_otf_unchanged() {
        let ttf = static_ttf();
        let font = FontRef::new(&ttf).unwrap();
        let mut builder = FontBuilder::new();
        for record in font.table_directory.table_records() {
            let tag = record.tag();
            builder.add_raw(tag, font.table_data(tag).unwrap().as_bytes().to_vec());
        }
        let svg = b"SVG table stand-in".to_vec();
        builder.add_raw(Tag::new(b"SVG "), svg.clone());

        let otf = ttf_to_otf(&builder.build()).unwrap();

        let converted = FontRef::new(&otf).unwrap();
        let copied = converted.table_data(Tag::new(b"SVG ")).unwrap();
        assert_eq!(copied.as_bytes(), svg.as_slice());
        assert!(converted.table_data(Tag::new(b"glyf")).is_none());
    }

    #[test]
    fn numbers_use_the_shortest_charstring_forms() {
        assert_eq!(charstring_number(0.0), [139]);
        assert_eq!(charstring_number(-107.0), [32]);
        assert_eq!(charstring_number(108.0), [247, 0]);
        assert_eq!(charstring_number(-108.0), [251, 0]);
        assert_eq!(charstring_number(1131.0), [250, 255]);
        assert_eq!(charstring_number(2000.0), [28, 0x07, 0xd0]);
        assert_eq!(charstring_number(0.5), [255, 0, 0, 0x80, 0]);
    }

    #[test]
    fn index_offsets_grow_with_the_data() {
        let small = index(&[vec![1, 2, 3]]).unwrap();
        assert_eq!(small, [0, 1, 1, 1, 4, 1, 2, 3]);

        let large = index(&[vec![0; 300]]).unwrap();
        assert_eq!(&large[0..3], [0, 1, 2]);
    }

    #[test]
    fn variable_fonts_are_refused() {
        let ttf = FontExporter::new()
            .compile_ttf(&sample_variable_font())
            .unwrap();

        assert!(matches!(ttf_to_otf(&ttf), Err(CffError::VariableFont)));
    }

    #[test]
    fn converted_font_has_cff_outlines_that_match_the_truetype_ones() {
        let ttf = static_ttf();
        let otf = ttf_to_otf(&ttf).unwrap();
        assert_eq!(&otf[0..4], b"OTTO");

        let source = SkrifaFont::new(&ttf).unwrap();
        let converted = SkrifaFont::new(&otf).unwrap();
        let reader = FontRef::new(&otf).unwrap();
        assert!(reader.table_data(Tag::new(b"CFF ")).is_some());
        assert!(reader.table_data(Tag::new(b"glyf")).is_none());
        assert!(reader.table_data(Tag::new(b"loca")).is_none());

        let count = reader.maxp().unwrap().num_glyphs();
        assert!(count > 1);
        for index in 0..u32::from(count) {
            let id = GlyphId::new(index);
            let before = outline_of(&source, id).bounding_box();
            let after = outline_of(&converted, id).bounding_box();
            assert!(
                (before.x0 - after.x0).abs() < 0.05
                    && (before.y0 - after.y0).abs() < 0.05
                    && (before.x1 - after.x1).abs() < 0.05
                    && (before.y1 - after.y1).abs() < 0.05,
                "glyph {index}: {before:?} against {after:?}"
            );

            let advance = |font: &SkrifaFont<'_>| {
                font.glyph_metrics(Size::unscaled(), LocationRef::default())
                    .advance_width(id)
            };
            assert_eq!(advance(&source), advance(&converted));
        }
    }

    #[test]
    fn converted_font_keeps_the_character_map_and_the_names() {
        let ttf = static_ttf();
        let otf = ttf_to_otf(&ttf).unwrap();

        let source = SkrifaFont::new(&ttf).unwrap();
        let converted = SkrifaFont::new(&otf).unwrap();
        let mappings = |font: &SkrifaFont<'_>| font.charmap().mappings().collect::<Vec<_>>();
        assert_eq!(mappings(&source), mappings(&converted));

        let name = |font: &SkrifaFont<'_>, id: u32| {
            font.glyph_names()
                .get(GlyphId::new(id))
                .map(|name| name.to_string())
        };
        for id in 0..u32::from(FontRef::new(&otf).unwrap().maxp().unwrap().num_glyphs()) {
            assert_eq!(name(&source, id), name(&converted, id), "glyph {id}");
        }
    }
}
