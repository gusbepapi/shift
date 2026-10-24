//! Loads Glyphs 4 sources through the Glyphs 3 shapes that `glyphs-reader` knows.
//!
//! `glyphs-reader` accepts format 3 only. A Glyphs 4 single file (`.formatVersion = 4`)
//! differs from format 3 in the places that matter to Shift:
//!
//! - The family name lives in the font `properties` (`familyNames`) instead of `familyName`.
//! - An instance names itself through its `properties` (`styleNames`) instead of `name`.
//! - A `Color Palettes` parameter is a dictionary of `names` and `palettes`, where each palette
//!   holds `colors`. Format 3 has a list of palettes, each a list of colours.
//! - Colours (`fillColor`, gradient stops and palette entries) are channels from 0 to 1 instead
//!   of integers from 0 to 255.
//!
//! The source is parsed as a plist, rewritten to those format 3 shapes, and handed to the reader
//! as text. A source that is not format 4 goes to the reader untouched. A `.glyphspackage` is
//! not rewritten, so a Glyphs 4 package still fails with the reader's own error.

use std::{fs, path::Path};

use glyphs_reader::{Font as GlyphsFont, Plist};
use ordered_float::OrderedFloat;

use crate::{ImportLoss, ImportLossKind};

/// Bytes of the file head searched for the format version, which Glyphs writes first.
const HEADER_BYTES: usize = 256;

const FORMAT_4_MARKER: &str = ".formatVersion = 4;";

const COLOR_PALETTES: &str = "Color Palettes";

/// A parsed Glyphs source with what the load could not carry over.
pub(super) struct LoadedSource {
    pub(super) font: GlyphsFont,
    pub(super) losses: Vec<ImportLoss>,
}

/// Loads a Glyphs source of format 2, 3 or 4.
///
/// # Errors
///
/// Returns the parser's message when the source cannot be read or parsed.
pub(super) fn load_source(path: &Path) -> Result<LoadedSource, String> {
    let text = match fs::read_to_string(path) {
        Ok(text) if is_format_4(&text) => text,
        _ => {
            return Ok(LoadedSource {
                font: GlyphsFont::load(path).map_err(|error| error.to_string())?,
                losses: Vec::new(),
            });
        }
    };
    let (rewritten, losses) = rewrite_to_format_3(&text)?;

    Ok(LoadedSource {
        font: GlyphsFont::load_from_string(&rewritten).map_err(|error| error.to_string())?,
        losses,
    })
}

fn is_format_4(text: &str) -> bool {
    let mut end = text.len().min(HEADER_BYTES);
    while !text.is_char_boundary(end) {
        end -= 1;
    }

    text[..end].contains(FORMAT_4_MARKER)
}

/// Rewrites Glyphs 4 source text to the format 3 shapes, listing what has no place there.
fn rewrite_to_format_3(text: &str) -> Result<(String, Vec<ImportLoss>), String> {
    let mut root = Plist::parse(text).map_err(|error| error.to_string())?;
    let Plist::Dictionary(font) = &mut root else {
        return Err("the Glyphs 4 source is not a dictionary".to_string());
    };
    let mut dropped_names = 0;

    font.insert(".formatVersion".into(), Plist::Integer(3));

    if !font.contains_key("familyName")
        && let Some(family) = font
            .get("properties")
            .and_then(|properties| default_property(properties, "familyNames"))
    {
        font.insert("familyName".into(), Plist::String(family));
    }

    if let Some(Plist::Array(instances)) = font.get_mut("instances") {
        for instance in instances {
            name_instance(instance);
        }
    }

    for value in font.values_mut() {
        convert_colors(value, &mut dropped_names);
    }

    let mut losses = Vec::new();
    if dropped_names > 0 {
        losses.push(ImportLoss {
            kind: ImportLossKind::Omitted,
            message: format!(
                "Shift does not keep Glyphs 4 colour palette entry names; {dropped_names} were omitted."
            ),
        });
    }

    Ok((root.to_string(), losses))
}

/// Gives an instance the `name` that format 3 requires, taken from its `styleNames`.
fn name_instance(instance: &mut Plist) {
    let Plist::Dictionary(instance) = instance else {
        return;
    };
    if instance.contains_key("name") {
        return;
    }

    let Some(name) = instance
        .get("properties")
        .and_then(|properties| default_property(properties, "styleNames"))
    else {
        return;
    };
    instance.insert("name".into(), Plist::String(name));
}

/// Reads the default-language value of a Glyphs 4 `properties` entry.
///
/// A `dflt` value wins, and the first value stands in when there is none.
fn default_property(properties: &Plist, key: &str) -> Option<String> {
    let property = properties
        .as_array()?
        .iter()
        .find(|property| property.get("key").and_then(Plist::as_str) == Some(key))?;
    let values = property.get("values")?.as_array()?;
    let value = values
        .iter()
        .find(|value| value.get("language").and_then(Plist::as_str) == Some("dflt"))
        .or_else(|| values.first())?;

    value.get("value")?.as_str().map(str::to_string)
}

/// Converts every colour below `value` from channels of 0 to 1 to integers from 0 to 255.
fn convert_colors(value: &mut Plist, dropped_names: &mut usize) {
    match value {
        Plist::Array(items) => {
            for item in items {
                convert_colors(item, dropped_names);
            }
        }
        Plist::Dictionary(dictionary) => {
            if dictionary.get("name").and_then(Plist::as_str) == Some(COLOR_PALETTES)
                && let Some(parameter) = dictionary.get_mut("value")
            {
                convert_palettes(parameter, dropped_names);
                return;
            }

            for (key, child) in dictionary.iter_mut() {
                match key.as_str() {
                    "fillColor" | "strokeColor" => convert_color(child),
                    "gradient" => convert_gradient(child),
                    _ => convert_colors(child, dropped_names),
                }
            }
        }
        _ => {}
    }
}

/// Rewrites a `Color Palettes` value from the Glyphs 4 dictionary to a list of palettes.
fn convert_palettes(parameter: &mut Plist, dropped_names: &mut usize) {
    let Plist::Dictionary(dictionary) = parameter else {
        return;
    };
    let Some(Plist::Array(palettes)) = dictionary.remove("palettes") else {
        return;
    };
    if let Some(Plist::Dictionary(names)) = dictionary.get("names") {
        *dropped_names += names.len();
    }

    let palettes = palettes
        .into_iter()
        .map(|palette| {
            let colors = match palette {
                Plist::Dictionary(mut palette) => palette.remove("colors"),
                other => Some(other),
            };
            let Some(Plist::Array(mut colors)) = colors else {
                return Plist::Array(Vec::new());
            };
            colors.iter_mut().for_each(convert_color);

            Plist::Array(colors)
        })
        .collect();

    *parameter = Plist::Array(palettes);
}

/// Converts the colour stops of a gradient, each a colour followed by its offset.
fn convert_gradient(gradient: &mut Plist) {
    let Plist::Dictionary(gradient) = gradient else {
        return;
    };
    let Some(Plist::Array(stops)) = gradient.get_mut("colors") else {
        return;
    };

    for stop in stops {
        if let Plist::Array(stop) = stop
            && let Some(color) = stop.first_mut()
        {
            convert_color(color);
        }
    }
}

/// Converts one grey-and-alpha or red-green-blue-alpha colour in place.
///
/// Anything else is left as it is, so the reader reports it.
fn convert_color(color: &mut Plist) {
    let Plist::Array(channels) = color else {
        return;
    };
    if !matches!(channels.len(), 2 | 4) {
        return;
    }

    let converted = channels.iter().map(channel).collect::<Option<Vec<_>>>();
    if let Some(converted) = converted {
        *channels = converted;
    }
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the channel is clamped to 0 to 1, so the scaled value fits a byte"
)]
fn channel(channel: &Plist) -> Option<Plist> {
    let value = match channel {
        Plist::Integer(value) => *value as f64,
        Plist::Float(OrderedFloat(value)) => *value,
        _ => return None,
    };

    Some(Plist::Integer(i64::from(
        (value.clamp(0.0, 1.0) * 255.0).round() as u8,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    const GLYPHS_4: &str = r#"{
.formatVersion = 4;
customParameters = (
{
name = "Color Palettes";
value = {
names = {
0 = Red;
1 = White;
};
palettes = (
{
colors = (
(1,0.21961,0.23529,1),
(1,1),
);
},
);
};
},
);
instances = (
{
properties = (
{
key = styleNames;
values = (
{
language = ENG;
value = Side;
},
{
language = dflt;
value = Front;
},
);
},
);
},
);
properties = (
{
key = familyNames;
values = (
{
language = dflt;
value = "Sample";
},
);
},
);
}"#;

    fn rewritten() -> (Plist, Vec<ImportLoss>) {
        let (text, losses) = rewrite_to_format_3(GLYPHS_4).unwrap();

        (Plist::parse(&text).unwrap(), losses)
    }

    #[test]
    fn detects_format_4_from_the_file_head_only() {
        assert!(is_format_4(GLYPHS_4));
        assert!(!is_format_4("{\n.formatVersion = 3;\n}"));
        assert!(!is_format_4(&format!(
            "{}{}",
            " ".repeat(300),
            FORMAT_4_MARKER
        )));
    }

    #[test]
    fn takes_the_family_and_instance_names_from_properties() {
        let (font, _) = rewritten();

        assert_eq!(font.get(".formatVersion").and_then(Plist::as_i64), Some(3));
        assert_eq!(
            font.get("familyName").and_then(Plist::as_str),
            Some("Sample")
        );
        let instance = &font.get("instances").unwrap().as_array().unwrap()[0];
        assert_eq!(instance.get("name").and_then(Plist::as_str), Some("Front"));
    }

    #[test]
    fn rewrites_the_palette_dictionary_to_integer_colours() {
        let (font, losses) = rewritten();

        let parameter = &font.get("customParameters").unwrap().as_array().unwrap()[0];
        let palettes = parameter.get("value").unwrap().as_array().unwrap();
        let colors = palettes[0]
            .as_array()
            .unwrap()
            .iter()
            .map(|color| {
                color
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|channel| channel.as_i64().unwrap())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        assert_eq!(colors, [vec![255, 56, 60, 255], vec![255, 255]]);
        assert_eq!(losses.len(), 1);
        assert!(losses[0].message.contains("2 were omitted"));
    }

    #[test]
    fn converts_layer_fill_colours_and_gradient_stops() {
        let mut layer = Plist::parse(
            "{ attr = { fillColor = (0.545,0.071,0.157,1); }; gradient = { colors = (((1,0,0,1),0),((0,0,1,1),1)); }; }",
        )
        .unwrap();

        convert_colors(&mut layer, &mut 0);

        let text = layer.to_string();
        assert!(text.contains("139"), "{text}");
        assert!(text.contains("255"), "{text}");
        assert!(!text.contains("0.545"), "{text}");
    }
}
