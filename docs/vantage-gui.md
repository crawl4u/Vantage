# Vantage GUI

The Vantage menu is moving off the mod-panel JSON path onto its own toolkit, `crates/vantage-gui`, so the
menu is limited by what we build and not by the host's mod-panel rules (document size, node and binding
counts, no gradients per element, no per-corner radii, no hover-only parts, no inline fields).

## Why

The redesign hit the mod-panel rules again and again: a page of 90 rows exceeded the 192 KiB document limit,
a card could have 32 nodes, bindings stopped at 256, the engine could not round only two corners, hover
could not reveal parts of a card, and a text field could only open the host's overlay editor. OneClient's menu
needs all of these (see the inventory below). Working around each one cost more than owning the layer.

## Shape

```
input (pointer, wheel, keys, text)      time
        │                                │
        ▼                                ▼
   vantage-gui: widget tree ──layout──▶ scene ──▶ painter
        ▲            │  animation / state            │
        │            ▼                               ▼
   Vantage settings  events (toggle, slider, …)   host draw calls
```

- **Widgets** hold state (hover, press, focus, scroll, animation) and build a **scene**: a flat list of
  primitives (rounded rectangle with per-corner radii, gradient fill, border, shadow, text, icon, vector path,
  image, clip). The scene is plain data and has no engine dependency, so every page is testable and renders
  offline.
- **Layout** is our own flex-style layout (row, column, stack, grid, wrap) in design units, resolved per
  frame against the viewport, so the menu scales and centres itself.
- **Input** is raw (pointer, buttons, wheel, keys, text); hit testing, hover, press, drag, focus and text
  editing live in the toolkit. Nothing is delegated to host controls.
- **Painter**: the scene is drawn by a thin painter in the client. Until a native pass exists, it emits the
  host's existing draw nodes through a trusted first-party path that skips the mod-panel limits; the scene
  never depends on that.

## Stages

1. Foundation (done in this change): geometry, style, scene, layout, animation, tests.
2. Painter: scene to host nodes (trusted path), per-corner radii by composition, gradients, shadows.
3. Widgets: button, toggle, slider with a value box, dropdown, chips, cards, text field, scroll view, grid,
   tooltip, colour picker, keybind capture, reorderable grid.
4. Pages: Mods, Profiles, Keybinds, Themes, Preferences, Changelog, Credits, Waypoints, mod settings, HUD
   editor, each matched against the OneClient recording.
5. Retire the JSON menu (`vantage-ui`, `menu_v2`) once every page is on the toolkit.

## OneClient inventory (from the 2026-11-23 recording)

- **Window**: translucent dark panel over the blurred world, large corner radius, no outline. A sidebar a
  little darker than the content, with the logo, an Edit HUD entry, grouped entries with icons and small
  upper-case group headings, a selected entry as a solid blue pill, and the player (avatar, name, game
  version, notification bell with a dot) at the bottom.
- **Header**: back and forward arrows, large page title, search pill with a magnifier, close.
- **Mods**: category chips with icons (All, Favorites, Performance, Visuals, HUD, Utility, Quality of Life,
  Other); icon sort control (grid, "Aa") with a tooltip; cards with a centred icon or the mod's name, a blue
  status dot, star and eye on hover, a name bar blue when on and grey when off; scrolling grid, empty states.
- **Profiles**: cards (new profile, default) with an actions bar (separate controls, open folder).
- **Keybinds**: a long list with status dots and conflict marks.
- **Screenshots**: sort chips, counts, an empty state with a button.
- **Themes**: preview cards for each theme, an accent colour row with a colour button, an animations switch.
- **Wardrobe**: tabs (Featured, Outfits, Skins, Emotes, Cosmetics), a skin list and a 3D preview of the player.
- **Preferences**: each setting a rounded row card with a title, a one-line description and either an
  On/Off switch or a slider with a stepper value box.
- **Changelog / Credits**: rich text with images, headings and lists; a large wordmark.
- **HUD editor**: a full-screen overlay with a title and subtitle preview, element selection, drag, resize
  and sounds.
- **Motion**: smooth transitions on hover, press, toggles, page changes, and menu open and close.

## Rules

- No engine types in `vantage-gui`; it depends on nothing but the standard library.
- Colours, sizes and timings are tokens in one place and read by every widget.
- Layout and animation take time from the caller; tests never sleep.
