# Java-style menus (design)

A second look for the client's out-of-game and pause menus, switched with a style setting, next
to the existing Bedrock (Ore UI) look. Horizon (horizon-rust/horizon, LGPL-3.0) is a visual and
behavioural reference only: no Horizon source is copied, so no LGPL code enters this repository.
The textures come from the player's own Java Edition install at runtime; none are shipped.

## Where it hooks in

- Out-of-game and pause screens are code-built Ore UI screens in `crates/oreui`
  (`home::draw`, `pause::draw`, the settings route), chosen in `crates/oreui/src/menu_screen.rs`
  and drawn on `paint::Canvas`. Navigation and state are `launcher::menu::{MenuView, MenuAction}`.
- A Java-style layer is a second set of `draw` functions on the same `Canvas`, reading the same
  `MenuView` and emitting the same `MenuAction` hits, so no menu logic is duplicated. The dispatch
  in `menu_screen.rs` picks the set from `MenuView`'s style.
- Settings are defined once in `crates/launcher/src/menu/settings_options/definitions.rs`
  (`SettingId`, section, kind). Java pages are generated from those definitions, so a setting
  with no Java counterpart still appears, on an automatic "More settings" page. Nothing is lost.

## Style setting and toggle

- New setting `menu_style` (Bedrock or Java), default Bedrock, stored with the other user
  settings and listed in Options.
- A button on the title screen of each style switches to the other (a `MenuAction`), so it works
  in game and from the start menu. The logo stays Vantage's in both styles.
- With no usable Java assets the Java style is unavailable: the toggle explains why and the
  Bedrock look stays.

## Java assets

- Find the player's Java Edition client jar (`.minecraft/versions/<version>/<version>.jar`, also
  the official launcher's default location), read only the GUI sprites the screens use
  (`widget/button*`, `widget/slider*`, `widget/checkbox*`, `widget/text_field*`, the menu list
  background and separators), and cache them in the local asset folder as an optional carrier.
  Fonts: the client's open font, not Mojang's.
- Nothing from the jar is committed or packaged, as with the Bedrock sample pack.

## Status

- Milestone 1 (committed): the `menu_style` setting (Options, General), a toggle in the top right
  of the home and pause screens (`crates/oreui/src/style_toggle.rs`), and the Java asset locator
  and sprite cache (`crates/launcher-host/src/java_ui.rs`), run at startup before the menu
  artwork loads and logged as `JAVA_UI`.
- Milestone 2: the cached sprites join the OreUI texture pages under `java/` keys
  (`crates/oreui/src/images/java.rs`, with their nine-slice borders), and `crates/oreui/src/java`
  draws the Java-style title screen (`java::home`) with nine-slice buttons. It draws only when
  the Java style is chosen and the sprites are installed. Check it offline with
  `cargo run -p ui-snapshot -- --state java-home --out home.png` (also `java-home-small`).
- Run the locator test against a real install with `VANTAGE_JAVA_DIR` set to a Java Edition
  folder (the test skips itself without it).

## Requirements (from the owner)

1. The menus use Java Edition's own bitmap font, drawn as the game draws it (8-pixel glyphs,
   per-glyph advance, one-pixel shadow). This matters a great deal.
2. The font and the widget sprites follow the selected resource pack, as in Java: a pack's
   replacement wins over the vanilla files from the Java install.
3. The menus work well with Bedrock: every Bedrock screen, setting and feature stays reachable,
   backed by the same state.
4. The menus behave exactly like Java's, not only look like them: keyboard navigation, focus and
   hover, press feedback and sounds, scrolling lists, sliders, text fields, Escape to go back.

## What following the selected pack needs

The client imports a Java pack by translating it into a Bedrock pack (`crates/java-resource-pack`)
and drops files Bedrock cannot use, which includes `assets/minecraft/textures/gui/sprites/widget`
and the font files. Following the pack therefore needs: (a) the translator keeping the Java GUI
and font files in the imported pack under a private folder; (b) one resolver that reads a Java UI
file from the active pack stack, top pack first, then from the vanilla cache; (c) rebuilding the
Java sprites and font when the active packs change, as the other pack-driven art does.

## Milestones

1. Style setting, toggle action and title-screen button; Java asset locator and cache. Done.
2. Java title screen with nine-slice widgets from the cached sprites. Done.
3. Java font. Done: glyph tables and sheets from the cached font files
   (`images/java_font.rs`), Java layout (advance, one-pixel quarter-colour shadow, centring on an
   8-pixel line) in `java/font.rs`, used by the Java title; the client's font is the fallback for
   characters the sheets lack, or when no Java font is installed.
4. Title fidelity. Done: Java's exact layout and scale rule (`java/home.rs`), the language and
   accessibility icon buttons that open Settings on those sections, the Bedrock-only buttons in
   the side margins, Java's splash list drawn rotated and pulsing (`java/splash.rs`), and Java's
   own panorama swapped in while the Java style is chosen. Java's files are read from the
   launcher's asset store first, then the jar, because the jar holds one-pixel stand-ins for
   some pictures. Still to do: Java's Unifont fallback for scripts the bitmap sheets lack, and
   the date-based splashes.
5. Resource pack following. Done: the importer keeps a pack's menu files under `javaui/`, one
   resolver (`images/java.rs`) reads each file from the selected packs and then the cache, and the
   Java menus' pages are a reserved block of the UI textures whose pixels change with the packs
   without changing the renderer's fixed identity. A pack button without its own borders takes
   the game's, scaled. Sheets up to 2048 pixels fit.
6. Pause menu. Done (`java/pause.rs`): Java's rows, holding what this client offers in place of
   advancements, statistics and the icon links.
7. Options pages. Done (`java/options.rs`): the list of pages and a page for each section with
   options in the settings registry, built from the registry's panels, with every other section
   and each page's "More settings..." reaching the Bedrock-style Settings. The view in use is
   `SettingsInteraction::java_view`; Back steps page, then list, then out.
8. Multiplayer list. Done (`java/servers.rs`): Java's entries, buttons and ping bars for saved and\n   featured servers; Join, Add, Edit and Delete use the existing server actions. Java's Direct\n   Connection and Refresh have no counterpart here yet.\n9. Own pages for what the registry cannot lay out. Done for Resource Packs (`java/packs.rs`:
   two lists, search, sort, activate, deactivate, move, delete after a question) and Shader Packs
   (`java/shaders.rs`); both share `java/frame.rs`. Still to do: language, mods, storage,
   account, touch, key bindings, and folders for packs.
10. Behaviour parity across the Java screens (the fourth requirement), checked screen by screen.

Later rounds: world list, create world, mods, the rest of the menus.
