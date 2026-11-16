//! Packs an SFNT binary into the WOFF web font container, which compresses
//! every table with zlib. WOFF2 is packed by the binary encodings instead.

use std::io::Write;

use flate2::Compression;
use flate2::write::ZlibEncoder;
use write_fonts::read::{FontRef, TableProvider};

/// Describes a failure to pack an SFNT binary into a web font container.
#[derive(Debug, thiserror::Error)]
pub enum WebFontError {
    #[error("cannot read the font to pack: {message}")]
    ReadFont { message: String },

    #[error("cannot compress the font: {message}")]
    Compress { message: String },

    #[error("the font is too large for the container: {value} {what}s do not fit")]
    TooLarge { value: usize, what: &'static str },
}

/// Converts a length to the integer type a container field stores.
///
/// # Errors
///
/// Returns [`WebFontError::TooLarge`] when `value` does not fit `T`.
fn narrow<T: TryFrom<usize>>(value: usize, what: &'static str) -> Result<T, WebFontError> {
    T::try_from(value).map_err(|_| WebFontError::TooLarge { value, what })
}

fn read_error(error: &impl std::fmt::Display) -> WebFontError {
    WebFontError::ReadFont {
        message: error.to_string(),
    }
}

fn compress_error(error: &impl std::fmt::Display) -> WebFontError {
    WebFontError::Compress {
        message: error.to_string(),
    }
}

/// One SFNT table as it sits in the source binary.
struct SourceTable<'a> {
    tag: [u8; 4],
    checksum: u32,
    data: &'a [u8],
}

struct Source<'a> {
    flavor: u32,
    version: (u16, u16),
    tables: Vec<SourceTable<'a>>,
}

impl<'a> Source<'a> {
    fn read(sfnt: &'a [u8]) -> Result<Self, WebFontError> {
        let font = FontRef::new(sfnt).map_err(|error| read_error(&error))?;
        let revision = font
            .head()
            .map_err(|error| read_error(&error))?
            .font_revision();
        let raw = revision.to_bits();

        let mut tables = Vec::new();
        for record in font.table_directory.table_records() {
            let tag = record.tag().to_be_bytes();
            let offset = record.offset() as usize;
            let length = record.length() as usize;
            let data = sfnt
                .get(offset..offset.saturating_add(length))
                .ok_or_else(|| WebFontError::ReadFont {
                    message: format!("table {} lies outside the file", tag_text(tag)),
                })?;
            tables.push(SourceTable {
                tag,
                checksum: record.checksum(),
                data,
            });
        }
        tables.sort_by_key(|table| table.tag);

        Ok(Self {
            flavor: font.table_directory.sfnt_version(),
            // The font revision is 16.16 fixed point; its halves give the container version
            version: {
                let [high_a, high_b, low_a, low_b] = raw.to_be_bytes();
                (
                    u16::from_be_bytes([high_a, high_b]),
                    u16::from_be_bytes([low_a, low_b]),
                )
            },
            tables,
        })
    }

    /// The size of the SFNT these tables came from, with every table padded to four bytes.
    ///
    /// # Errors
    ///
    /// Returns [`WebFontError::TooLarge`] when the size exceeds `u32::MAX`.
    fn sfnt_size(&self) -> Result<u32, WebFontError> {
        let directory = 12 + 16 * self.tables.len();
        let data: usize = self
            .tables
            .iter()
            .map(|table| padded(table.data.len()))
            .sum();
        narrow(directory + data, "byte")
    }
}

fn tag_text(tag: [u8; 4]) -> String {
    String::from_utf8_lossy(&tag).into_owned()
}

fn padded(length: usize) -> usize {
    length.div_ceil(4) * 4
}

fn push_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn pad_to_four(out: &mut Vec<u8>) {
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}

/// Packs an SFNT binary into a WOFF 1.0 file.
///
/// # Errors
///
/// Returns [`WebFontError`] when the binary cannot be read or a table cannot be compressed.
pub(crate) fn to_woff(sfnt: &[u8]) -> Result<Vec<u8>, WebFontError> {
    const HEADER: usize = 44;
    const ENTRY: usize = 20;

    let source = Source::read(sfnt)?;

    // A table stays uncompressed when compression would not make it smaller
    let mut stored = Vec::with_capacity(source.tables.len());
    for table in &source.tables {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
        encoder
            .write_all(table.data)
            .map_err(|error| compress_error(&error))?;
        let compressed = encoder.finish().map_err(|error| compress_error(&error))?;
        stored.push(if compressed.len() < table.data.len() {
            compressed
        } else {
            table.data.to_vec()
        });
    }

    let mut offset = HEADER + ENTRY * source.tables.len();
    let mut directory = Vec::new();
    for (table, data) in source.tables.iter().zip(&stored) {
        directory.extend_from_slice(&table.tag);
        push_u32(&mut directory, narrow(offset, "byte")?);
        push_u32(&mut directory, narrow(data.len(), "byte")?);
        push_u32(&mut directory, narrow(table.data.len(), "byte")?);
        push_u32(&mut directory, table.checksum);
        offset += padded(data.len());
    }

    let mut out = Vec::with_capacity(offset);
    out.extend_from_slice(b"wOFF");
    push_u32(&mut out, source.flavor);
    push_u32(&mut out, narrow(offset, "byte")?);
    push_u16(&mut out, narrow(source.tables.len(), "table")?);
    push_u16(&mut out, 0);
    push_u32(&mut out, source.sfnt_size()?);
    push_u16(&mut out, source.version.0);
    push_u16(&mut out, source.version.1);
    // No metadata block and no private data
    for _ in 0..5 {
        push_u32(&mut out, 0);
    }
    out.extend_from_slice(&directory);
    for data in &stored {
        out.extend_from_slice(data);
        pad_to_four(&mut out);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::read::ZlibDecoder;
    use std::io::Read;

    fn u32_at(bytes: &[u8], offset: usize) -> u32 {
        u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap())
    }

    fn u16_at(bytes: &[u8], offset: usize) -> u16 {
        u16::from_be_bytes(bytes[offset..offset + 2].try_into().unwrap())
    }

    /// Compiles a small static font to use as a container payload.
    fn sample_sfnt() -> Vec<u8> {
        use crate::FontExporter;
        use shift_font::test_support::sample_variable_font;

        FontExporter::new()
            .compile_ttf(&sample_variable_font())
            .unwrap()
    }

    #[test]
    fn woff_tables_decompress_to_the_source_tables() {
        let sfnt = sample_sfnt();
        let woff = to_woff(&sfnt).unwrap();

        assert_eq!(&woff[0..4], b"wOFF");
        assert_eq!(u32_at(&woff, 8) as usize, woff.len());
        assert!(woff.len().is_multiple_of(4));

        let source = Source::read(&sfnt).unwrap();
        let count = u16_at(&woff, 12) as usize;
        assert_eq!(count, source.tables.len());
        assert_eq!(u32_at(&woff, 16), source.sfnt_size().unwrap());

        for (index, table) in source.tables.iter().enumerate() {
            let entry = 44 + 20 * index;
            assert_eq!(&woff[entry..entry + 4], &table.tag);
            let offset = u32_at(&woff, entry + 4) as usize;
            let compressed = u32_at(&woff, entry + 8) as usize;
            let original = u32_at(&woff, entry + 12) as usize;
            assert_eq!(original, table.data.len());
            assert_eq!(u32_at(&woff, entry + 16), table.checksum);

            let stored = &woff[offset..offset + compressed];
            let restored = if compressed < original {
                let mut restored = Vec::new();
                ZlibDecoder::new(stored).read_to_end(&mut restored).unwrap();
                restored
            } else {
                stored.to_vec()
            };
            assert_eq!(restored, table.data, "table {}", tag_text(table.tag));
        }
    }
}
