//! Colour authoring: palettes and the layered paint graphs of colour glyphs.

use miette::{Result, bail, miette};
use shift_font::{
    ColorGlyph, ColorPalette, ColorPoint, ColorStop, FontIntent, FontIntentSet, Paint, PaintColor,
    RemoveColorGlyph, Rgba8, SetColorGlyph, SetColorPalettes,
};

use crate::cli::{RemoveColorGlyphArgs, SetColorGlyphArgs, SetPalettesArgs};

use crate::selector::FontSelectors;

use super::{AuthoringReport, apply_mutation, glyph::load_font};

/// Replaces every palette with the palettes given on the command line.
///
/// # Errors
///
/// Returns an error for malformed colours, palettes of unequal length, a
/// palette that would leave a paint pointing at a missing entry, or a
/// document/persistence failure.
pub(crate) fn set_palettes(args: &SetPalettesArgs) -> Result<AuthoringReport> {
    if args.palettes.is_empty() && !args.clear {
        bail!("color palette set requires --palette or --clear");
    }
    let palettes = args
        .palettes
        .iter()
        .map(|text| parse_palette(text))
        .collect::<Result<Vec<_>>>()?;
    let intent = SetColorPalettes::new(&palettes).map_err(|error| miette!("{error}"))?;
    apply_mutation(
        &args.path,
        &args.mutation,
        &FontIntentSet {
            intents: vec![FontIntent::SetColorPalettes(intent)],
        },
    )
}

/// Creates or replaces the layers painting one base glyph.
///
/// # Errors
///
/// Returns an error for unknown glyphs, malformed fills, paints that use
/// palette entries that do not exist, or a document/persistence failure.
pub(crate) fn set_color_glyph(args: &SetColorGlyphArgs) -> Result<AuthoringReport> {
    let font = load_font(&args.path)?;
    let glyph = font.glyph_id_for(&args.glyph)?;
    let layers = args
        .layers
        .iter()
        .map(|text| {
            let (name, fill) = text
                .split_once('=')
                .ok_or_else(|| miette!("layer {text:?} must be GLYPH=FILL"))?;
            Ok(Paint::Glyph {
                glyph: font.glyph_id_for(name)?,
                fill: Box::new(parse_fill(fill)?),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let intent = SetColorGlyph::new(&ColorGlyph {
        glyph,
        paint: Paint::Layers { layers },
    })
    .map_err(|error| miette!("{error}"))?;
    apply_mutation(
        &args.path,
        &args.mutation,
        &FontIntentSet {
            intents: vec![FontIntent::SetColorGlyph(intent)],
        },
    )
}

/// Removes the colour of one base glyph, leaving its outline.
///
/// # Errors
///
/// Returns an error for an unknown glyph, a glyph without colour, or a
/// document/persistence failure.
pub(crate) fn remove_color_glyph(args: &RemoveColorGlyphArgs) -> Result<AuthoringReport> {
    let font = load_font(&args.path)?;
    let glyph_id = font.glyph_id_for(&args.glyph)?;
    apply_mutation(
        &args.path,
        &args.mutation,
        &FontIntentSet {
            intents: vec![FontIntent::RemoveColorGlyph(RemoveColorGlyph { glyph_id })],
        },
    )
}

/// Parses `[NAME:]#RRGGBB[AA],#RRGGBB[AA],...`.
fn parse_palette(text: &str) -> Result<ColorPalette> {
    let (name, colors) = match text.split_once(':') {
        Some((name, colors)) if !name.starts_with('#') => (Some(name.to_string()), colors),
        _ => (None, text),
    };
    let colors = colors
        .split(',')
        .map(|color| parse_hex(color.trim()))
        .collect::<Result<Vec<_>>>()?;
    Ok(ColorPalette { name, colors })
}

fn parse_hex(text: &str) -> Result<Rgba8> {
    let digits = text
        .strip_prefix('#')
        .ok_or_else(|| miette!("colour {text:?} must start with #"))?;
    if !matches!(digits.len(), 6 | 8) || !digits.chars().all(|digit| digit.is_ascii_hexdigit()) {
        bail!("colour {text:?} must be #RRGGBB or #RRGGBBAA");
    }
    let value = u32::from_str_radix(digits, 16)
        .map_err(|_error| miette!("colour {text:?} is not hexadecimal"))?;
    let opaque_value = if digits.len() == 8 {
        value
    } else {
        (value << 8) | 0xFF
    };
    let [red, green, blue, alpha] = opaque_value.to_be_bytes();

    Ok(Rgba8::new(red, green, blue, alpha))
}

/// Parses a layer fill; the grammar is documented on `--layer`.
fn parse_fill(text: &str) -> Result<Paint> {
    if let Some(rest) = text.strip_prefix("linear:") {
        let ([x0, y0, x1, y1], stops) = split_geometry(rest)?;
        return Ok(Paint::LinearGradient {
            stops: parse_stops(stops)?,
            p0: ColorPoint::new(x0, y0),
            p1: ColorPoint::new(x1, y1),
            p2: None,
        });
    }
    if let Some(rest) = text.strip_prefix("radial:") {
        let ([x0, y0, r0, x1, y1, r1], stops) = split_geometry(rest)?;
        return Ok(Paint::RadialGradient {
            stops: parse_stops(stops)?,
            c0: ColorPoint::new(x0, y0),
            r0: Some(r0),
            c1: ColorPoint::new(x1, y1),
            r1: Some(r1),
        });
    }
    let (color, alpha) = parse_color_alpha(text)?;
    Ok(Paint::Solid { color, alpha })
}

fn split_geometry<const COUNT: usize>(text: &str) -> Result<([f64; COUNT], &str)> {
    let (geometry, stops) = text
        .split_once(':')
        .ok_or_else(|| miette!("gradient {text:?} needs GEOMETRY:STOPS"))?;
    let numbers = geometry
        .split(',')
        .map(|number| {
            number
                .trim()
                .parse::<f64>()
                .map_err(|_error| miette!("gradient coordinate {number:?} is not a number"))
        })
        .collect::<Result<Vec<_>>>()?;
    let found = numbers.len();
    let Ok(numbers) = <[f64; COUNT]>::try_from(numbers) else {
        bail!("gradient geometry {geometry:?} needs {COUNT} numbers, found {found}");
    };

    Ok((numbers, stops))
}

fn parse_stops(text: &str) -> Result<Vec<ColorStop>> {
    text.split('/')
        .map(|stop| {
            let (offset, color) = stop
                .split_once('=')
                .ok_or_else(|| miette!("stop {stop:?} must be OFFSET=COLOUR"))?;
            let (color, alpha) = parse_color_alpha(color)?;
            Ok(ColorStop {
                offset: offset
                    .trim()
                    .parse::<f64>()
                    .map_err(|_error| miette!("stop offset {offset:?} is not a number"))?,
                color,
                alpha,
            })
        })
        .collect()
}

/// Parses `palette:INDEX[@ALPHA]` or `foreground[@ALPHA]`.
fn parse_color_alpha(text: &str) -> Result<(PaintColor, f64)> {
    let (color, alpha) = match text.split_once('@') {
        Some((color, alpha)) => (
            color,
            alpha
                .parse::<f64>()
                .map_err(|_error| miette!("alpha {alpha:?} is not a number"))?,
        ),
        None => (text, 1.0),
    };
    let color = if color == "foreground" {
        PaintColor::Foreground
    } else if let Some(index) = color.strip_prefix("palette:") {
        PaintColor::Palette {
            index: index
                .parse()
                .map_err(|_error| miette!("palette index {index:?} is not a whole number"))?,
        }
    } else {
        bail!("fill {text:?} must be palette:INDEX, foreground, linear:... or radial:...");
    };
    Ok((color, alpha))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_palettes_with_and_without_names() {
        let named = parse_palette("Dark:#102030,#ff000080").unwrap();
        assert_eq!(named.name.as_deref(), Some("Dark"));
        assert_eq!(
            named.colors,
            vec![Rgba8::new(16, 32, 48, 255), Rgba8::new(255, 0, 0, 128)]
        );

        let anonymous = parse_palette("#ffffff").unwrap();
        assert_eq!(anonymous.name, None);
        assert_eq!(anonymous.colors, vec![Rgba8::new(255, 255, 255, 255)]);

        parse_palette("#12345").unwrap_err();
        parse_palette("red").unwrap_err();
    }

    #[test]
    fn parses_solid_and_gradient_fills() {
        assert_eq!(
            parse_fill("palette:2@0.5").unwrap(),
            Paint::Solid {
                color: PaintColor::Palette { index: 2 },
                alpha: 0.5
            }
        );
        assert_eq!(
            parse_fill("foreground").unwrap(),
            Paint::Solid {
                color: PaintColor::Foreground,
                alpha: 1.0
            }
        );
        let Paint::LinearGradient { stops, p0, p1, .. } =
            parse_fill("linear:0,0,0,700:0=palette:0/1=palette:1@0.5").unwrap()
        else {
            panic!("expected a linear gradient");
        };
        assert_eq!(
            (p0, p1),
            (ColorPoint::new(0.0, 0.0), ColorPoint::new(0.0, 700.0))
        );
        assert_eq!(stops.len(), 2);
        assert_eq!(stops[1].alpha, 0.5);

        let Paint::RadialGradient { r0, r1, .. } =
            parse_fill("radial:0,0,0,0,0,300:0=foreground").unwrap()
        else {
            panic!("expected a radial gradient");
        };
        assert_eq!((r0, r1), (Some(0.0), Some(300.0)));

        parse_fill("linear:0,0,0:0=foreground").unwrap_err();
        parse_fill("blue").unwrap_err();
        parse_fill("palette:x").unwrap_err();
    }
}
