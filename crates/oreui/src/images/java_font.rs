//! Java Edition's bitmap font: which glyph sits where in each sheet, how wide it is and how far
//! the pen moves after it, worked out from the font definitions and the sheets' pixels the way
//! the game works them out.

use std::collections::HashMap;

/// A decoded bitmap sheet, straight RGBA8.
pub struct Sheet {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Sheet {
    fn alpha(&self, x: u32, y: u32) -> u8 {
        self.rgba[(y as usize * self.width as usize + x as usize) * 4 + 3]
    }
}

/// One glyph of the font.
#[derive(Clone, Debug, PartialEq)]
pub struct Glyph {
    /// The sheet's name, such as `ascii.png`; none for a glyph that only moves the pen.
    pub sheet: Option<String>,
    /// The ink's rectangle in the sheet, in texels: x, y, width, height.
    pub cell: [u16; 4],
    /// Java pixels the pen moves after the glyph.
    pub advance: f32,
    /// Java pixels the ink is drawn wide and tall.
    pub size: [f32; 2],
    /// Java pixels from the line's top to the ink's top; negative is above it.
    pub top: f32,
}

/// The glyphs of every sheet and space definition, by character.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct JavaFont {
    glyphs: HashMap<char, Glyph>,
}

impl JavaFont {
    /// The glyph for `character`, if the font has one.
    #[must_use]
    pub fn glyph(&self, character: char) -> Option<&Glyph> {
        self.glyphs.get(&character)
    }

    /// Whether the font holds no glyphs at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.glyphs.is_empty()
    }

    /// Builds the font from definition files, in priority order, and the sheets they name. The first
    /// definition that has a character keeps it, as in the game.
    #[must_use]
    pub fn build(definitions: &[&[u8]], sheets: &HashMap<String, Sheet>) -> Self {
        let mut font = Self::default();
        for definition in definitions {
            let Ok(json) = serde_json::from_slice::<serde_json::Value>(definition) else {
                continue;
            };
            for provider in json["providers"].as_array().into_iter().flatten() {
                match provider["type"].as_str() {
                    Some("space") => font.add_spaces(provider),
                    Some("bitmap") => font.add_bitmap(provider, sheets),
                    _ => {}
                }
            }
        }
        font
    }

    fn add_spaces(&mut self, provider: &serde_json::Value) {
        for (key, advance) in provider["advances"].as_object().into_iter().flatten() {
            let (mut characters, Some(advance)) = (key.chars(), advance.as_f64()) else {
                continue;
            };
            if let (Some(character), None) = (characters.next(), characters.next()) {
                self.glyphs.entry(character).or_insert(Glyph {
                    sheet: None,
                    cell: [0; 4],
                    advance: advance as f32,
                    size: [0.0; 2],
                    top: 0.0,
                });
            }
        }
    }

    fn add_bitmap(&mut self, provider: &serde_json::Value, sheets: &HashMap<String, Sheet>) {
        let Some(name) = provider["file"]
            .as_str()
            .and_then(|file| file.strip_prefix("minecraft:font/"))
        else {
            return;
        };
        let Some(sheet) = sheets.get(name) else {
            return;
        };
        let rows: Vec<Vec<char>> = provider["chars"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|row| row.as_str().map(|row| row.chars().collect()))
            .collect();
        let columns = rows.first().map_or(0, Vec::len) as u32;
        if rows.is_empty() || columns == 0 || rows.iter().any(|row| row.len() as u32 != columns) {
            return;
        }
        let (cell_width, cell_height) = (sheet.width / columns, sheet.height / rows.len() as u32);
        if cell_width == 0 || cell_height == 0 {
            return;
        }
        let height = provider["height"].as_f64().unwrap_or(8.0) as f32;
        let ascent = provider["ascent"].as_f64().unwrap_or(0.0) as f32;
        let scale = height / cell_height as f32;
        for (row, characters) in rows.iter().enumerate() {
            for (column, character) in characters.iter().enumerate() {
                if *character == '\0' {
                    continue;
                }
                let (left, top) = (column as u32 * cell_width, row as u32 * cell_height);
                // The ink ends at the rightmost column that has any pixel.
                let ink = (0..cell_width)
                    .rev()
                    .find(|x| (0..cell_height).any(|y| sheet.alpha(left + x, top + y) != 0));
                let Some(ink) = ink else { continue };
                let width = ink + 1;
                self.glyphs.entry(*character).or_insert(Glyph {
                    sheet: Some(name.to_owned()),
                    cell: [left as u16, top as u16, width as u16, cell_height as u16],
                    advance: ((0.5 + width as f32 * scale) as i32) as f32 + 1.0,
                    size: [width as f32 * scale, height],
                    top: 7.0 - ascent,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sheet of `columns` by `rows` cells of `cell` texels, with ink in the listed columns of
    /// the listed cells (cell index = row * columns + column).
    fn sheet(columns: u32, rows: u32, cell: [u32; 2], ink: &[(u32, &[u32])]) -> Sheet {
        let (width, height) = (columns * cell[0], rows * cell[1]);
        let mut rgba = vec![0; (width * height * 4) as usize];
        for (index, inked_columns) in ink {
            let (left, top) = ((index % columns) * cell[0], (index / columns) * cell[1]);
            for x in *inked_columns {
                let at = (((top + 2) * width + left + x) * 4 + 3) as usize;
                rgba[at] = 255;
            }
        }
        Sheet {
            width,
            height,
            rgba,
        }
    }

    fn build(definition: &str, name: &str, sheet: Sheet) -> JavaFont {
        JavaFont::build(
            &[definition.as_bytes()],
            &HashMap::from([(name.to_owned(), sheet)]),
        )
    }

    #[test]
    fn a_glyph_is_as_wide_as_its_rightmost_inked_column_and_advances_one_more() {
        let font = build(
            r#"{"providers":[{"type":"bitmap","file":"minecraft:font/a.png","ascent":7,"chars":["ab"]}]}"#,
            "a.png",
            sheet(2, 1, [8, 8], &[(0, &[0, 3]), (1, &[0, 1, 2, 5])]),
        );
        let a = font.glyph('a').unwrap();
        assert_eq!(
            (a.cell, a.advance, a.size, a.top),
            ([0, 0, 4, 8], 5.0, [4.0, 8.0], 0.0)
        );
        let b = font.glyph('b').unwrap();
        assert_eq!((b.cell, b.advance), ([8, 0, 6, 8], 7.0));
    }

    #[test]
    fn a_cell_without_ink_and_a_nul_have_no_glyph() {
        let font = build(
            r#"{"providers":[{"type":"bitmap","file":"minecraft:font/a.png","ascent":7,"chars":["a\u0000b"]}]}"#,
            "a.png",
            sheet(3, 1, [8, 8], &[(0, &[1]), (1, &[2])]),
        );
        assert!(font.glyph('a').is_some());
        assert!(font.glyph('\0').is_none());
        assert!(font.glyph('b').is_none());
    }

    #[test]
    fn taller_cells_scale_to_their_height_and_sit_above_the_line_by_their_ascent() {
        let font = build(
            r#"{"providers":[{"type":"bitmap","file":"minecraft:font/t.png","height":12,"ascent":10,"chars":["é"]}]}"#,
            "t.png",
            sheet(1, 1, [9, 12], &[(0, &[0, 6])]),
        );
        let glyph = font.glyph('é').unwrap();
        assert_eq!(
            (glyph.size, glyph.top, glyph.advance),
            ([7.0, 12.0], -3.0, 8.0)
        );
    }

    #[test]
    fn the_first_definition_keeps_a_character_and_spaces_only_advance() {
        let sheets = HashMap::from([
            ("a.png".to_owned(), sheet(1, 1, [8, 8], &[(0, &[2])])),
            ("b.png".to_owned(), sheet(1, 1, [8, 8], &[(0, &[6])])),
        ]);
        let space = r#"{"providers":[{"type":"space","advances":{" ":4,"‌":0}}]}"#.as_bytes();
        let first = r#"{"providers":[{"type":"bitmap","file":"minecraft:font/a.png","ascent":7,"chars":["x"]}]}"#.as_bytes();
        let second = r#"{"providers":[{"type":"bitmap","file":"minecraft:font/b.png","ascent":7,"chars":["x"]}]}"#.as_bytes();
        let font = JavaFont::build(&[space, first, second], &sheets);
        assert_eq!(font.glyph('x').unwrap().advance, 4.0);
        let gap = font.glyph(' ').unwrap();
        assert_eq!((gap.sheet.clone(), gap.advance), (None, 4.0));
        assert_eq!(font.glyph('\u{200c}').unwrap().advance, 0.0);
    }

    #[test]
    fn bad_definitions_and_missing_sheets_leave_an_empty_font() {
        assert!(JavaFont::build(&[b"not json".as_slice()], &HashMap::new()).is_empty());
        let missing = r#"{"providers":[{"type":"bitmap","file":"minecraft:font/none.png","ascent":7,"chars":["a"]}]}"#.as_bytes();
        assert!(JavaFont::build(&[missing], &HashMap::new()).is_empty());
    }
}
