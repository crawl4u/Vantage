# Vantage UI overhaul: reference and plan

The Vantage menu is being rebuilt to follow the structure and feel of the in-game menu of
OneClient (OneConfig, by Polyfrost). This file records what was observed and how the rebuild is
planned. It is a specification of behaviour and measurements, not a translation.

## How the reference is used

OneConfig is published under the LGPL-3 with Polyfrost's additional terms. The Vantage menu is
written from scratch in Rust:

- No OneConfig code is ported or transliterated, and none of its assets (icons, fonts, art,
  sounds, strings, branding or names) are copied. Icons come from the Lucide set Vantage already
  ships (`crates/vantage/LICENSE-LUCIDE`); text is written fresh.
- Only observable design facts are carried over: layout structure, proportions, spacing, colour
  values, interaction rules and states.
- The shipped menu does not use the OneConfig or OneClient names or marks.

## Observed design

### Shell

- A single dark, translucent panel with a left sidebar and a content pane. Sidebar width 264.
- The sidebar holds a brand mark at the top (28 above it), grouped navigation sections with
  collapsible headings, an overflow line ("+N more") when entries are clipped, an options strip
  and an account row at the bottom (player head, name, notification bell).
- Entries: Edit HUD, Global search, then sections Mods & options (Mods, Profiles, Keybinds),
  Personalization (Themes, Preferences) and a final section (Changelog, Feedback).
- The content pane has a page gutter of 25 and a header with back/forward arrows, the page title,
  a search field and a close button.

### Mods page

- A row of filter chips (not a tab bar): All, Favourites, Hypixel, Performance, Visuals, HUD,
  Utility, Quality of life, Other, Hidden. Chips for empty categories are hidden; the active chip
  always shows; clicking the active chip returns to All. Hidden shows only when something is hidden.
- A sort control at the top right of the page.
- A grid of cards, 4 columns, 19 gap both ways, 25 side padding, with a soft fade at the scroll
  edges. Cards can be grouped under collapsible type headers that span the row.
- A card is an icon tile with the mod name on a bar along the bottom. The bar is filled with the
  accent when the mod is on and neutral when off; a small dot marks the card corner. Cards can
  be favourited (star), hidden, toggled from the card, and dragged to reorder within their
  group. Dragging lifts the card into an overlay and the grid re-lays out under the pointer.
  Pressing a card opens that mod's settings page.
- Empty Favourites and Hidden views show a centred icon, a title and one line of help.

### Theme

- Three styles: flat, glass and pixel. Default is a dark glass look ("frosted"): panel fill about
  `#11171C` at 77% opacity, secondary surface `#151C22` at 70%, raised surface `#232D32` at 70%,
  dim surface `#232D32` at 35%, opaque deep fields `#1A2229`; hairline border white at 10%;
  text `#D5DBFF`; muted text `#757883`; highlight text white; gold accent for stars `#FFD700`.
  A light variant swaps these for pale blue-greys. A solid "game" variant drops the transparency.
- Shapes: small, medium, large, field, pill and circle radii; inner shapes are concentric (inner
  radius is outer minus inset).
- Type scale (px): display 24, title large 22, title 18, heading 15, body 14, label 13, small 12,
  caption 11, eyebrow 11 with 1.5 letter spacing, mono 12. Weights: regular, medium, semi-bold.
- Spacing scale: 2, 4, 6, 8, 12, 16, 24, page gutter 25.
- Text legibility: any text colour is lightened or darkened toward black or white until it reaches
  4.5:1 contrast against its surface.
- Motion: placement animations on grid changes, eased hover, and a reduced-motion setting that
  turns animation off.

### Settings page for one mod

Each option is one row with its label and control. Controls seen: toggle, slider, number, range,
text, dropdown, multi-select dropdown, radio group, colour (and colour list), keybind (with
conflict warning and recording), file, button, info text, draggable list, and lists of items,
numbers, text, effects, tiles and files. Options can be grouped into collapsible sections and
sub-menus, searched by name, reset to default, and inherited from a shared value.

### Other pages

Profiles (named setting sets), Keybinds (grouped, collapsible, with a visual keyboard map and
conflict marking), Themes (pick and a custom theme editor), Preferences, Changelog, Credits and
a global search results page. A separate HUD editor lets elements be dragged with snapping,
measured, grouped, undone, and previewed; vanilla HUD footprints show as ghosts.

## Where it lands in Vantage

The Vantage menu today is a sidebar of 27 pages with about 900 controls
(`crates/vantage/src/menu_controls.rs`, `menu.json`, `panel.rs`). The mod only sends values and
events; the host draws the panel from JSON-UI with mod-supplied appearance, so the look is the
mod's to define (see `AGENTS.md`: the host provides no finished widgets).

## Plan

Each phase ships on its own and keeps every existing setting working.

1. **Design tokens.** One module holds the palette, radii, type scale and spacing above, with the
   contrast rule as a tested function. Everything below reads from it.
2. **Shell.** Sidebar, header, search field, close, and the page frame. Existing pages render
   inside the new frame unchanged at first.
3. **Mods page.** Category chips, the 4-column card grid, on/off bar, favourite, hide, sort and
   the empty states, backed by the existing mod list (`MODS` in `menu_controls.rs`).
4. **Option rows.** The shared row layout and each control type, replacing the page-by-page
   widgets; grouped and collapsible sections; reset to default; search by option name.
5. **Pages.** Move each of the 27 pages onto the option rows, then Profiles (existing setups),
   Keybinds and a Themes page.
6. **HUD editor and polish.** Drag with snapping, motion settings, reduced motion.

Open questions are recorded in the pull request that starts each phase.

## Status

Done: tokens, shell, the Mods page on the mod's real list, and option rows. In the live menu (`new_menu`,
on by default) the Mods tab and 22 settings pages (Layout, Style, Chunk borders, Zoom, Crosshair, General,
Hitboxes, Hit color, Time changer, Weather changer, Nametags, Hurt camera, Damage indicator, Menu blur,
CPS limiter, Block outline, Markers, Text hotkey, Hive, Perspective, Glass, Sharing) use the new design;
their rows come from the controls each old page pressed, under the old control ids.

Still on the old design: Config (setups), Credits and Waypoints. Not yet built: a dropdown list for
choices (they cycle), colour pickers, icons (cards show an initial), hiding a row until its parent option
is on, and the Crosshair preview.
