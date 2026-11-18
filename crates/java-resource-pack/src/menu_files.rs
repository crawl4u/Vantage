//! The Java Edition files the Java-style menus draw from, named once for the three places that
//! handle them: the extractor that copies them out of the player's game, the importer that keeps a
//! Java resource pack's copies, and the menus that read them. Every path is Java's own, relative
//! to `assets/minecraft/`, so a pack's file and the game's file for one name are interchangeable
//! and a pack's wins, as in Java.

/// The folder inside the compiled assets folder where the player's Java files are cached.
pub const CACHE_FOLDER: &str = "java-ui";

/// Where the GUI files below sit, relative to `assets/minecraft/`.
pub const GUI_ROOT: &str = "textures/gui/";

/// The GUI sprites and backgrounds the menus draw, relative to [`GUI_ROOT`]. Each may have a
/// `.mcmeta` beside it that sets its nine-slice borders.
pub const GUI_FILES: &[&str] = &[
    "sprites/widget/button.png",
    "sprites/widget/button_highlighted.png",
    "sprites/widget/button_disabled.png",
    "sprites/widget/slider.png",
    "sprites/widget/slider_highlighted.png",
    "sprites/widget/slider_handle.png",
    "sprites/widget/slider_handle_highlighted.png",
    "sprites/widget/checkbox.png",
    "sprites/widget/checkbox_highlighted.png",
    "sprites/widget/checkbox_selected.png",
    "sprites/widget/checkbox_selected_highlighted.png",
    "sprites/server_list/ping_1.png",
    "sprites/server_list/ping_2.png",
    "sprites/server_list/ping_3.png",
    "sprites/server_list/ping_4.png",
    "sprites/server_list/ping_5.png",
    "sprites/server_list/ping_unreachable.png",
    "sprites/server_list/pinging_1.png",
    "sprites/widget/scroller.png",
    "sprites/widget/scroller_background.png",
    "sprites/widget/text_field.png",
    "sprites/widget/text_field_highlighted.png",
    "sprites/icon/language.png",
    "sprites/icon/accessibility.png",
    "menu_list_background.png",
    "inworld_menu_list_background.png",
    "header_separator.png",
    "footer_separator.png",
    "inworld_header_separator.png",
    "inworld_footer_separator.png",
];

/// Sprites without which the Java style cannot draw its controls, relative to [`GUI_ROOT`].
pub const REQUIRED_GUI_FILES: &[&str] = &[
    "sprites/widget/button.png",
    "sprites/widget/button_highlighted.png",
    "sprites/widget/button_disabled.png",
];

/// The font definitions the default font is read from, in the order the game reads them. Packs
/// add their own `font/default.json`; their providers come before these.
pub const FONT_DEFINITIONS: &[&str] = &[
    "font/default.json",
    "font/include/space.json",
    "font/include/default.json",
];

/// Where the bitmap sheets a font definition names sit, relative to `assets/minecraft/`.
pub const FONT_TEXTURES: &str = "textures/font/";

/// The title panorama's faces, relative to `assets/minecraft/`.
pub const PANORAMA_FACES: [&str; 6] = [
    "textures/gui/title/background/panorama_0.png",
    "textures/gui/title/background/panorama_1.png",
    "textures/gui/title/background/panorama_2.png",
    "textures/gui/title/background/panorama_3.png",
    "textures/gui/title/background/panorama_4.png",
    "textures/gui/title/background/panorama_5.png",
];

/// The title's splash lines, relative to `assets/minecraft/`.
pub const SPLASHES: &str = "texts/splashes.txt";

/// Where the importer keeps a Java pack's menu files inside the Bedrock pack it translates the
/// pack into: this prefix, then the file's path relative to `assets/minecraft/`.
pub const PACK_PREFIX: &str = "javaui/";

/// The path of `gui_file` (relative to [`GUI_ROOT`]) relative to `assets/minecraft/`.
#[must_use]
pub fn gui_path(gui_file: &str) -> String {
    format!("{GUI_ROOT}{gui_file}")
}

/// The path of the `.mcmeta` beside `path`.
#[must_use]
pub fn mcmeta(path: &str) -> String {
    format!("{path}.mcmeta")
}

/// Whether `path`, relative to `assets/minecraft/`, is a file the menus can use from a Java
/// pack: a listed GUI file or its `.mcmeta`, a font definition or bitmap sheet, a panorama face
/// or the splash lines. Everything else in a pack is left to the Bedrock translation.
#[must_use]
pub fn is_menu_file(path: &str) -> bool {
    let plain = path.strip_suffix(".mcmeta").unwrap_or(path);
    if let Some(gui) = plain.strip_prefix(GUI_ROOT) {
        return GUI_FILES.contains(&gui) || PANORAMA_FACES.contains(&plain);
    }
    if path == SPLASHES {
        return true;
    }
    // Font definitions sit in `font/` or `font/include/`, sheets directly in `textures/font/`.
    if let Some(name) = path
        .strip_prefix("font/include/")
        .or_else(|| path.strip_prefix("font/"))
    {
        return name.ends_with(".json") && !name.contains('/') && !name.contains("..");
    }
    path.strip_prefix(FONT_TEXTURES).is_some_and(|name| {
        name.ends_with(".png")
            && !name.contains('/')
            && !name.contains('\\')
            && !name.contains("..")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_gui_file_and_its_metadata_is_a_menu_file() {
        for file in GUI_FILES {
            let path = gui_path(file);
            assert!(is_menu_file(&path), "{path}");
            assert!(is_menu_file(&mcmeta(&path)), "{path}.mcmeta");
        }
        for file in REQUIRED_GUI_FILES {
            assert!(GUI_FILES.contains(file), "{file} must also be listed");
        }
    }

    #[test]
    fn fonts_panorama_and_splashes_are_menu_files_and_other_files_are_not() {
        for path in [
            "font/default.json",
            "font/include/default.json",
            "textures/font/ascii.png",
            "textures/gui/title/background/panorama_3.png",
            "texts/splashes.txt",
        ] {
            assert!(is_menu_file(path), "{path}");
        }
        for path in [
            "textures/block/stone.png",
            "textures/gui/sprites/widget/unlisted.png",
            "textures/gui/title/background/panorama_6.png",
            "textures/font/sub/ascii.png",
            "textures/font/../../x.png",
            "font/sub/x.json",
            "font/ascii.png",
            "texts/credits.txt",
        ] {
            assert!(!is_menu_file(path), "{path}");
        }
    }
}
