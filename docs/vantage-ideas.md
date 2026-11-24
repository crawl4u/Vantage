# Vantage menu: features to add later

A running list of things the menu work has shown we should build. Add to it as new gaps appear; remove an item when
it ships. Items are grouped by where they live, not ranked.

## Mods page and card previews

- Inventory card: draw each stack's enchantment glint and durability bar, and let the sample items follow the player's
  own hotbar contents (an opt-in "show my items" switch).
- Potion status card: show several effects with their levels and the blink of an effect about to run out, in the
  chosen Normal or Minimal mode.
- Cards for every remaining Glyph mod (see `mod_icons.rs`): render the mod's real output where it has one, such as the
  hitbox, block outline and light-spot overlays.
- Item icons for any pack texture (not only items): a general "pack texture" primitive with tinting, so cards can
  show effect, particle and GUI sprites.
- Card previews follow the element's saved style (colour, opacity, size) instead of its defaults.

## Preferences

Preferences now lists the theme, the menu key, snap to guides, centre lines and the menu blur look. Settings that
should join it:

- Menu scale (the window's size independent of the game's GUI scale) and a compact layout.
- Reduced motion: shorter or no open, close and page animations.
- Menu sounds (clicks and hover) with a volume.
- Language and number or time format (12 or 24 hour) applied to every HUD element.
- Notification centre: which events post a notice (updates, profile loads, share codes).
- Default values for new elements: shadow, rounded corners, text colour and background opacity.
- Remember the last open page and scroll position between sessions (the page is remembered within a session now).
- Search: remember recent searches and filters.

## Pages that are placeholders

- Screenshots: a browser of the game's screenshots folder with open, copy and delete.
- Wardrobe: choose and preview cosmetics from the player's account.
- Changelog: release notes shown in the menu.
- Themes: the page shows a card for Vantage Dark and Vantage Light. Still to add: more palettes (the reference page has
  four cards), an Accent colour row with a colour picker, and an Animations switch (on: the open, close and page
  animations; off: reduced motion). Both rows need new saved settings, and the card previews should follow the accent.
- Sidebar footer: the player's name and head from the signed-in account.

## HUD editor

The editor has a scene-drawn toolbar (Lines, Grid, HUDs, Settings, undo, redo, Reset, Done); a library of HUD cards with a search field (click a card to add it, or drag it onto the screen); a dock with Layout, Style and Settings tabs and a search over its rows; an action bar over the picked HUD (lock, settings, remove and a size badge); a right-click context menu; a guided tour on first open; a shortcuts pill and sheet; round handles; outlines that are brighter under the pointer and brightest once picked; a marquee, Shift clicks and groups for picking several HUDs, which then move and nudge together; a grid the HUDs snap to; Alt to measure the gap between two HUDs; arrow-key nudging; copy and paste of a HUD's look (Ctrl+C, Ctrl+V); undo and redo of up to 100 layouts; and keys for G, Delete and the Ctrl shortcuts. Still to build from the reference editor (decided: these wait for a later date):

- Duplicating a HUD: each HUD exists once today, so a copy needs more than one instance of a HUD.
- Snapping to equal gaps, with guide lines and gap markers.
- Merging touching HUD backgrounds into one shape.
- Anchoring one HUD to a point of another, so its place follows.
- Hiding a HUD without taking it off the screen, apart from the lock.
- Resizing several picked HUDs together, and the dock editing all of them at once (it follows the one the editor is on).
- The tour pointing at the parts of the editor it talks about.

## Menu toolkit

- Scroll bars on every scrolling area, with dragging (the Mods grid and option pages have it).
- Keyboard navigation of the grid and the option rows.
- A shared way to time page builds in a test without the wall clock, so a slow page fails CI (the General page took
  125 ms a frame before its keybind owners were looked up once).
- The mod API's item and texture drawing primitives, once a first-party mod needs them outside the menu.
