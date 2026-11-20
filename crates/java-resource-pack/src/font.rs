//! A Java pack's bitmap font as the Bedrock sheet the client already layers over its own font.
//!
//! Java's `textures/font/ascii.png` is a 16 by 16 grid whose cells hold the ASCII range from its
//! third row and the code page 437 characters from its ninth, the same places Bedrock's
//! `font/default8.png` holds them. The sheet goes out under that name, with the cells Java leaves
//! empty cleared, and a marker so the client keeps its own glyph wherever the pack draws none
//! instead of blanking the character.

use image::RgbaImage;

use crate::texture::{Budget, duplicate};

/// The sheet's path in a Bedrock pack.
pub(crate) const SHEET: &str = "font/default8.png";
/// Tells the client the pack's blank cells mean "not drawn here" rather than "draw nothing".
pub(crate) const MARKER: &str = "font/cinnabar_font.json";
pub(crate) const MARKER_BYTES: &[u8] = br#"{"blank_cells_keep_base":true}"#;

/// Cells of the grid's first two rows and its last cell of the ASCII rows, which Java leaves for
/// control characters and Bedrock fills with accented letters and punctuation.
const FIRST_DRAWN_ROW: u32 = 2;
const DELETE_CELL: u32 = 127;
const GRID: u32 = 16;

/// The Bedrock sheet for Java's ASCII sheet, or `None` when `ascii` is not a grid of cells.
pub(crate) fn default8(ascii: &RgbaImage, budget: &Budget) -> Option<RgbaImage> {
    let (width, height) = ascii.dimensions();
    if width == 0 || height == 0 || width % GRID != 0 || height % GRID != 0 {
        return None;
    }
    let (cell_width, cell_height) = (width / GRID, height / GRID);
    let mut sheet = duplicate(ascii, budget)?;
    for (x, y, pixel) in sheet.enumerate_pixels_mut() {
        let cell = y / cell_height * GRID + x / cell_width;
        if y / cell_height < FIRST_DRAWN_ROW || cell == DELETE_CELL {
            pixel.0 = [0; 4];
        }
    }
    Some(sheet)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn the_ascii_and_code_page_cells_carry_over_and_the_control_rows_are_cleared() {
        let mut ascii = RgbaImage::new(32, 32);
        let ink = Rgba([255, 255, 255, 255]);
        // Cells are 2 by 2: one in the first row, one for `A` (65: row 4, column 1), one at 127.
        for (cell_x, cell_y) in [(3, 0), (1, 4), (15, 7), (2, 9)] {
            ascii.put_pixel(cell_x * 2, cell_y * 2, ink);
        }
        let sheet = default8(&ascii, &Budget::new()).unwrap();
        assert_eq!(sheet.get_pixel(6, 0).0[3], 0, "control row");
        assert_eq!(sheet.get_pixel(2, 8).0, ink.0, "A");
        assert_eq!(sheet.get_pixel(30, 14).0[3], 0, "delete");
        assert_eq!(sheet.get_pixel(4, 18).0, ink.0, "code page 437");
    }

    #[test]
    fn a_sheet_that_is_not_a_sixteen_grid_gives_nothing() {
        assert!(default8(&RgbaImage::new(30, 32), &Budget::new()).is_none());
        assert!(default8(&RgbaImage::new(0, 0), &Budget::new()).is_none());
    }
}
