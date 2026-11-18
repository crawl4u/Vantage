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

- Milestone 1 is in: the `menu_style` setting (Options, General), a toggle button in the top
  right of the home and pause screens (`crates/oreui/src/style_toggle.rs`), and the Java asset
  locator and sprite cache (`crates/launcher-host/src/java_ui.rs`, run in the background at
  startup, logged as `JAVA_UI`). Choosing Java does not change how the menus draw yet; the Java
  renderer is milestone 2.
- Run the locator test against a real install with `VANTAGE_JAVA_DIR` set to a Java Edition
  folder (the test skips itself without it).

## Milestones

1. Style setting, toggle action and title-screen button; Java asset locator and cache.
2. Java widget kit (nine-slice buttons, sliders, checkboxes, text fields, list backgrounds) and the
   Java-style title screen, with `ui-snapshot` states to check it offline.
3. Java-style pause menu.
4. Java-style Options pages generated from the settings definitions, plus "More settings".

Later rounds: world and server lists, create world, mods, the rest of the menus.
