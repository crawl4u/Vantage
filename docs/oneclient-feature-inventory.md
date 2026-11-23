# OneClient / OneConfig in-game menu: feature and behaviour inventory

Purpose: a clean-room behavioural specification so the menu can be re-implemented from scratch in Rust.
This is a description of behaviour, structure and numbers written in our own words. No source code, strings
(other than short option and page identifiers), icon files or assets were copied. OneConfig is LGPL-3.0 with
extra terms; do not paste its code, text blobs, SVGs or fonts into this repository. Fonts named below
(Poppins, a Minecraft-style pixel font) have their own licences; source our own copies independently.

All dimensions are in "dp", a logical pixel at the menu's design size (see section 1). Colours are written
`0xAARRGGBB`. "Ms" is milliseconds.

## 0. Scope, sources and an important caveat

Sources read (public GitHub, branch `v1` of Polyfrost/OneConfig, HEAD at about changelog 1.9.3 dated
2026-11-23): the UI module (`modules/ui`), the internal UI implementation (`modules/internal/.../ui/...`:
shell, sidebar, header, screens, theme, HUD editor, search, sound, settings controls), the preferences and
theme config classes, the English language file, `CHANGELOG.md`, and the config manager. Also read: the
Polyfrost/OneLauncher release notes for `oneclient-2.11.0` (2026-11-15) and the Polyfrost/OneClient mod
README.

Findings that change the premise of the task:

1. "OneClient 2.11.0" is a launcher release (OneLauncher repo). Its notes are about the launcher (instance
   settings, mod downloads, cluster sharing), not about an in-game menu. The in-game menu a player sees is
   OneConfig's own UI, which OneClient installs as a mod. The "OneClient" mod repo itself only replaces the
   title screen (logo over panorama; Singleplayer, Multiplayer, Realms, Settings, Mods; fullscreen/quit
   buttons; one click back to the vanilla menu) and targets Minecraft 26.3 on Fabric.
2. Which OneConfig version OneClient 2.11.0 bundles is not stated anywhere I could read. Tags visible:
   `v1.6.1` (2026-11-02), `v1.6.2+post.1`. The `v1` branch is already at 1.9.x (HUD groups added in 1.9.1,
   Quick Search and keyboard map in 1.8.0). Treat what follows as "OneConfig v1 HEAD"; anything introduced
   after 1.6.x may not exist in the build the user compares against. Items I could date from the changelog are
   marked with the version.
3. There is **no Screenshots page and no Wardrobe/cosmetics page** in OneConfig. The seven built-in pages are
   Mods, Profiles, Keybinds, Themes, Preferences, Changelog, Credits (plus the HUD editor as a separate
   screen). Searching the repository tree for "wardrobe", "screenshot", "cosmetic" returned nothing. Any
   wardrobe or screenshot UI lives in other Polyfrost products (PolyPlus, the launcher) and was not
   investigated. The credits page mentions a "lead cosmetics designer, OneClient" role only.
4. The rendering stack is Compose-style retained UI (PolyCompose since 1.9.0) with navigation back stack,
   springs and tweens. Names like "TabStyle.Chips" refer to that toolkit; the behaviours are what matter.

## 1. Window, scaling, open and close

### 1.1 Shell geometry
- Design size of the whole shell: **1391 x 700 dp**. It is always drawn at this size and then scaled as a unit.
- Scale rule: `scale = max(0.25, min(1, 0.9 * windowW / (1391 * density), 0.9 * windowH / (700 * density)))`.
  So the menu never exceeds 1.0 at the host density and always leaves a 5 percent margin per side, and it
  never goes below a quarter size.
- User "UI Scale" preference (50 to 200 percent, step 5, default 100) multiplies on top. At exactly 100 it
  uses the fitted scale. Changes are debounced by 400 ms before the density is applied (so dragging the
  slider does not thrash layout).
- "Pixel Grid" preference (Automatic / Always / Never; default Automatic). When active (Always, or Automatic
  with a pixel-style theme) the scale is snapped so a 14 sp anchor font height is a multiple of 5 device
  pixels (minimum 10 px), keeping pixel fonts sharp.
- Shell is centred in the window. Page content has a 25 dp horizontal gutter; vertical padding of the content
  column is 19 dp, and the gap between header and page is 19 dp (0 while a mods search is showing results).
- Layout: left sidebar 264 dp wide, 1 dp divider, right content column. Overall corner radius 20 dp (theme
  `LG`), 1 dp border in the theme border colour, drop shadow (when theme has shadow): blur 50, spread 4, offset
  (0, 13), opacity 0.42.

### 1.2 Backdrop
- Behind the panel the game is optionally blurred ("Background Blur", default on; off when Reduce Transparency
  is on). Blur intensity animates with the open animation (`easeOutExpo` of elapsed/250 ms on open; scaled by
  the close duration on close). With animations off the blur is simply 1 while open and 0 once closing.
- Accent glow: three large radial glows (extent 1400 px radius, nominal 500) drawn behind the panels, tinted by
  the accent at "Background Glow Opacity" (default 25, 0 to 100). Each drifts on slow sine paths (periods
  17 to 31 s, drift 60 to 110 px), "breathes" (alpha varies by 18 to 30 percent on 9 to 13 s periods) and
  shifts against the pointer for parallax (depth 0.04, 0.07, 0.11; approach rate 4 per second, dt clamped to
  0.1 s). Glow centres in shell pixels: (147, -199) strength 1, (893, 736) strength 1, (1330, 110) strength
  0.55. The radial falloff profile (position to alpha): 0 to 0.95, 0.2 to 0.77, 0.4 to 0.42, 0.5 to 0.25,
  0.6 to 0.13, 0.7 to 0.055, 0.8 to 0.018, 1.0 to 0. Glow is clipped separately to sidebar and page areas, then
  a fill colour (sidebar or page background) is drawn on top, so glow only shows through the translucent fill.
  Glow is disabled for PolyGlass themes, with Reduced Motion (drawn once, static), and with Reduce
  Transparency (zero).
- Panel fill opacity: sidebar default 80, page default 88 (preferences, 0 to 100). For flat themes the slider
  value replaces the colour's alpha outright (percent/100). For glass themes the slider scales relative to the
  default: below default it scales alpha linearly to 0; above default it interpolates from the colour's alpha
  to 1.
- "Reduce Transparency" makes all surfaces opaque (each translucent surface is composited over the opaque page
  colour) and turns off blur and glow.

### 1.3 Open and close
- Open: shell fades and scales from **0.9 to 1.0** over `Opening Time` seconds (default 0.6 s, range 0.05 to
  2.0, step 0.05) with an ease-out-exponential curve (`1 - 2^(-10x)`). If "Opening Animation" is off, 1 ms.
- Close: same fade/scale in reverse over the same time but **capped at 160 ms**, ease-out-cubic. Disabled by
  "Closing Animation" off. All durations are 0 when global "Animations" is off.
- Interrupting a close (pressing the menu key during the close animation) cancels it, resumes the blur from
  its current intensity and replays the open sound.
- Sounds: an Open sound on show, a Close sound on begin-close; ambience loop starts on open and stops on
  close (see 12).
- Escape closes the menu (unless a popup, dialog or capturing control handles it first). The OneConfig key
  (default Right Shift) either opens the HUD editor from the menu or, when "Keybind Closes Menu" is on (default
  off), closes it. Pressing it while minimal mode is active opens the HUD editor. Side or extra mouse buttons
  can also be the menu bind. While a keybind-capture field is recording, the menu key does nothing.
- Menu can pause singleplayer ("Pause Game", default off).
- When the menu closes, configs are saved on a serialized background path (two closes cannot write the same
  files at once); the time of closing is recorded (see page memory below).
- The shell is warmed in the background before first open (several frames rendered into an offscreen scene),
  purely as a hitch-avoidance measure; not user-visible.

### 1.4 Which page opens
Preference "Remember Last Page" (default "15 seconds"): Never, 15 seconds, 1 minute, 5 minutes, Always. If the
menu was closed within that window the last top-level page reopens (with its search text preserved and no
transition); otherwise Mods opens. The HUD editor is never restored as a page unless "Reopen HUD Editor"
(default off) is on. First open per session uses the "Show Opening Page Animation" preference (default off);
later navigations use "Show Page Animations" (default on). If "Instant Search" is on (default on) the search
field takes focus when the menu opens or the page changes.

### 1.5 Minimal "Quick Search" mode (1.8.0)
- Opened by the Quick Search keybind (default Backslash). A Spotlight-style panel: width 760 dp, top offset
  120 dp from the top of the design area, content padding 12 dp, same corner radius, border and surface fill as
  the page. Contains a 48 dp-high extra-large search bar (16 sp, with the brand icon, 20 dp, as the leading
  glyph and a clear button when text is present), then, only when the query is non-blank, a results area
  440 dp high (2-column result grid), then a footer row with two small actions: "Settings" (cog icon; opens the
  full menu on Preferences) and "Edit HUD" (hud icon; opens the editor) showing the bound OneConfig key as a
  keycap hint.
- Any navigation (including opening a result) expands the shell into the full menu. The nav graph is kept
  composed off-screen so a result can navigate into it.

## 2. Navigation model

- Pages, in order, with sidebar group, icon id and order value:
  | id | title | icon | group | order |
  |---|---|---|---|---|
  | mods | Mods | settings | Mods & Options | 100 |
  | profiles | Profiles | profiles | Mods & Options | 200 |
  | keybinds | Keybinds | keyboard | Mods & Options | 300 |
  | themes | Themes | paintbrush | Personalization | 100 |
  | preferences | Preferences | cog | Personalization | 200 |
  | changelog | Changelog | refresh | OneConfig (about menu) | 100 |
  | credits | Credits | star | OneConfig (about menu) | 200 |
  Other mods can register extra pages (id `namespace:key`); the `oneconfig` namespace is reserved and a
  third-party page claiming it is replaced by the built-in. Registered pages with `showInSidebar` appear in
  their group.
- Each mod also has a dynamic route (`ModConfig`, id plus optional submenu path and category) for its
  settings; unknown ids or unregistered pages fall back to Mods with a log entry.
- History: independent back and forward stacks. Navigating to a different route pushes the current entry
  (route, selected settings tab, search query, page argument) on the back stack and clears forward. Navigating
  to the same route is a no-op. Back/forward arrow buttons are 32 dp ghost icon buttons (icons left-arrow,
  right-arrow), disabled when the stack is empty. Switching settings tabs within a page is also recorded as
  history, so Back undoes a tab change without leaving the page. Search text is stored per history entry and
  restored on back or forward; ordinary navigation clears it.
- Clicking the sidebar entry of the page you are already on does not navigate; it clears the search text.
- The last selected tab per page is remembered across navigations. Scroll position is remembered per page and
  per category/search key (and restored for mods grid and lists).
- Arguments can be delivered to a page (one-shot payload with a 10 second expiry, retained while the page
  stays composed).
- "Jump to option": global search results navigate to a mod's page, switch to the right category tab, expand
  any group holding the option, scroll it into view, and play a highlight (below). A later navigation of any
  kind clears a pending jump.
- Page transition: horizontal slide by **one fifth of the content width** plus cross-fade over
  `Page Animation Duration` (default 0.3 s, range 0.1 to 0.6, step 0.05). Forward navigation: new page enters
  from the right, old exits left; back reverses. Reduced Motion removes the slide and keeps the fade. Tab
  switches inside a page use the same slide. "Show Page Animations" off makes all of it instant. Header title
  cross-fades.
- Header (36 dp high row; sits at the top of the content column with horizontal gutter 25): left side has back
  and forward arrows (8 dp apart), then 24 dp gap, then the title. Title is a breadcrumb for mod pages:
  `Mod name / Submenu / Subsubmenu`, 24 sp, semi-bold-to-medium; earlier crumbs are clickable (secondary
  colour, primary on hover, 150 ms colour tween), "/" separators have 6 dp padding. After the title: a
  secondary-colour byline ("by FirstAuthor" or "by FirstAuthor and N more...") and a 14 dp info glyph in a
  22 dp box whose tooltip lists Author, Credits and Version. If the mod offers its original (non-OneConfig)
  screen a small primary "Original menu" button (external-link icon) fades in beside it with a tooltip warning
  that the original screen may show options OneConfig doesn't. Right side: global search field (256 dp wide)
  and a 32 dp close (X) icon button, 26 dp apart.
- Sidebar details are in section 4.

## 3. Shared interaction and visual primitives

### 3.1 Control sizes and metrics
Heights: Compact 28, Regular 32, Large 38, ExtraLarge 45 dp. Horizontal padding 10/16/16/18, font 13/14/14/16 sp,
icon 14/16/16/20 dp. Control corner radius uses the theme "button" shape. Spacing scale: xxs 2, xs 4, s 6, m 8,
l 12, xl 16, xxl 24; page gutter 25.

### 3.2 Radii (flat/glass themes; pixel themes are all 0)
XS 6 (checkbox, slider thumb, swatch), SM 8 (buttons, chips, list entries, sidebar nav entries), MD 16
(setting rows, mod cards, changelog cards), LG 20 (popups, dropdown menus, list containers, window). Rule for
nested shapes: inner radius = outer radius minus inset, clamped at 0 ("concentric"). Mod icon shape default 4,
scrollbar shape 8.

### 3.3 Typography (Poppins in flat/glass themes; pixel font in Minecraft themes)
title 18 semi-bold; heading 15 semi-bold; body 14 regular; label 13 medium; small 12; caption 11; eyebrow 11
medium with 1.5 sp letter spacing (section headers); mono 12. Derived: titleSmall 16 medium, titleLarge 22
medium, display 24 semi-bold (header), plus medium-weight variants of body/small/heading/caption/title and a
regular-weight label. Setting row title uses heading-medium, description uses regular label. Poppins weights
shipped: Thin, ExtraLight, Light, Regular, Medium, SemiBold, ExtraBold, Black. Minecraft themes ship Regular
and Bold of a pixel font only.

### 3.4 Interaction state values
- Disabled alpha 0.5. Hover overlay alpha 0.06 (light) or 0.08 (dark); pressed overlay 0.12 (light) or 0.14
  (dark). Slider muted fill alpha 0.4, muted thumb 0.5, sheen top 0.20 to bottom 0.04.
- Default animation: 150 ms tween ("standard"); "fast" is 120 ms. Colour, float and dp state animations all
  use 150 ms unless stated. Item reflow in grids uses a medium-low-stiffness spring. When Reduced Motion is on
  (or Animations off), springs and tweens snap, and enters/exits become 150 ms fades.
- Tooltip: delay **300 ms** after hover (or focus) before showing; max width 240 dp (description tooltips 260);
  gap 8 dp from anchor, 8 dp from window edge; prefers the requested placement (Above/Below x Start/Center/End)
  and flips to the other side if it does not fit; horizontally clamped into the window. A tooltip does not
  re-pop over a dialog its button just opened.
- Focus ring colour: accent (flat/glass) or black/white (pixel). Every interactive control exposes a role and
  label to the accessibility/narrator layer.
- Confirm-on-click ("armed") buttons: first click changes the label to a "Click again to ..." style
  confirmation; an arm delay of 400 ms before a second click counts; disarms after 3000 ms. Confirm dialogs
  exist too (title, text, confirm label, optional icon, optional danger styling).
- Toasts: default duration 4000 ms, max 3 visible at once, max width 360 dp, 16 dp margin, 1 dp border.
- Context menus: right-click on cards, option rows, theme cards, profile cards, notifications, text fields.
  Items have icon, label, optional detail text (right side), enabled/disabled with a reason, danger style,
  separators. Disabled items show the reason.
- Mouse wheel scrolling uses an eased glide: each wheel notch is 96 dp, animates over 0.2 s with ease-start
  exponent 2, accumulating (max start factor 3); with Reduced Motion it jumps. Scroll containers add a 32 dp
  fade at the top and bottom edges. Scroll containers contain wheel events (an inner list at its end does not
  chain to the outer one). Scrollbar is thin with the theme scrollbar shape.
- Keyboard: Tab focus traversal over controls; cards and the sort control opt out of focus (cards are
  `canFocus=false` so a scroll-back never slides focus into view). Alt+Up and Alt+Down move a focused mod card
  or list entry (1.8.0). Escape unwinds: capture mode, popups, dialogs, then the menu.
- Pointer cursor: hand over clickable text-links.

### 3.5 Icon names used by the menu (identifier list; art must be original)
cog, settings, profiles, keyboard, paintbrush, refresh, star, star-filled, hud, hypixel, lightning-01, qol,
combat, cloud, bell, info, search, close, left-arrow, right-arrow, up, down, plus, minus, tick, check-circle,
x-circle, alert-circle, warning, copy, copy-plus, clipboard, scissors, select-all, trash, lock, unlock, eye,
eye-off, external-link, upload, dots-grid, dots-vertical, capitalized, uppercase, lowercase, bold, italic,
underline, strikethrough, text, text-input, align, align-left, align-center, align-right, layers, group,
ungroup, maximise, move, moon, sun, shuffle, spanner, folder, console, preferences. Pixel (Minecraft) themes
override most of these with blocky variants; unknown ones fall back to the normal icon. The sidebar and
mod-card glyph colour follows the text colour; icon tint is the accent for "info" notices.

## 4. Sidebar

- Width 264 dp; fill is the theme sidebar colour at the sidebar opacity; a 1 dp divider on its right edge.
- Top: wordmark/logo, **167 x 19 dp**, drawn with the accent gradient fill (logo asset is theme-specific: a
  normal one, a light one, a white one for PolyGlass, and pixel variants), 28 dp from the top.
- Navigation list: 28 dp below the logo; entries are 225 dp wide centred in the 264 dp column (19.5 dp
  inset); entry rows spaced 8 dp; groups spaced 18 dp. Each group has a collapsible section header (uppercase
  eyebrow style, text colour at 0.8 alpha, chevron). Collapse state is in-memory per group (not persisted);
  collapsing animates vertical expand/shrink. Group title shows only if the group has a title.
- Selected entry: accent-filled pill (flat themes); hover and pressed overlays otherwise. Entry = icon + label.
  The list scrolls; if entries are scrolled out of view a "+N more" text link (caption size, alpha 0.7, 1.0 on
  hover) appears and animates a scroll to the bottom. A reserved 74 dp gap sits above the footer.
- "Edit HUD" entry (hud icon) is its own section just above the account footer; it opens the HUD editor.
- Footer (73 dp tall, 1 dp top border): 32 dp player head (shape = sidebar entry shape), player name (1 line,
  ellipsis) and below it, in caption secondary colour, the game/version label (falls back to "OneConfig").
  Right side, 4 dp apart: an **About** icon button (info icon, Large size) that opens a dropdown menu above it
  listing the two about pages (Changelog and Credits, with icons and a check on the current one), and a
  **Notifications bell** (Large). The bell opens an anchored panel above it; unread count > 0 draws an 8 dp
  accent dot with a 7 dp blur at the bell's top-right (offset +2, -2). Opening the panel marks all read when it
  closes.
- About pages (Changelog, Credits) are not shown in the main sidebar list.

## 5. Mods page

### 5.1 Layout
- Top row: category chips on the left (gutter 25), **sort control** (segmented icon tabs) on the right (top
  aligned, right gutter 25). Below, the card grid fills the remaining height. 19 dp vertical gap between rows.
- Grid: **4 fixed columns**, 19 dp gap both axes, 25 dp horizontal content padding, scroll fade 32 dp. Cards are
  full column width and **140 dp high**.
- Placement animations when order changes use a spring; they are only enabled after the first layout at the
  current window size (to avoid animating the initial layout).
- Clicking empty grid space unfocuses the search field.

### 5.2 Category chips
Chips (chip style): All, Favorites (star icon), Hypixel, Performance (lightning-01), Visuals (paintbrush), HUD
(hud), Utility (settings), Quality of Life (qol), Other, Hidden (eye-off). A chip is shown only if: it is All or
Favorites (always), or it is currently selected, or (Hidden) at least one mod is hidden, or (others) at least
one non-hidden mod has that category. Clicking the selected chip deselects back to All (single-select with
deselect). Third-party mods without a declared category are categorised by a lookup list (unverified detail).
Each mod config carries one category from: combat, qol, hypixel, other, performance, visuals, hud, utility, info,
player, compat; only the nine chip categories above filter.

### 5.3 Filtering and ordering
- A card is shown when `hidden == (selected chip is Hidden)`; category chips filter on category; Favorites
  filters favourites; All shows every non-hidden mod. Search text on this page filters by title or tags (fuzzy,
  see 11); while a search query is non-blank the whole page body is replaced by the global search results
  screen.
- Order key (stable multi-key sort): (1) card **type rank** (optional groups registered by other mods, ordered
  by descending priority then case-insensitive title then id; untyped = -1 so they come first), (2) favourites
  first, (3) custom order index (skipped in A to Z mode; mods never dragged are unknown and sort alphabetically
  ahead of dragged ones), (4) HUD-only cards after non-HUD ones, (5) case-insensitive title.
- Type groups: contiguous runs of a given type get a full-width header row: a thin divider, optional 14 dp
  icon, the title (11 sp medium, secondary colour), a "(count)" suffix when collapsed, a chevron, another
  divider. Clicking toggles collapse; collapse state is persisted per type id. Collapsed groups hide their
  cards.
- Empty states: Hidden chip with nothing hidden shows an eye-off icon, a title and a hint saying that hiding a
  mod from its card puts it here. Favorites with none shows a star icon, title and hint to star a card.

### 5.3a Sort control
Segmented icon tabs, Large size, tooltips below: **Custom order** (dots-grid icon) and **A to Z**
(capitalized icon), plus any registered extra sort modes (each with id, icon, title, optional confirmation
text and confirm label). Selecting a mode with confirmation text first opens an anchored confirm popup (below,
end-aligned) "Sort by X?" + the text + a primary confirm button (default label "Switch"). Selected sort id is
persisted in a one-line file `mod-sort` (default `custom`). Dragging a card while in A to Z mode does not
reorder; when it reorders in custom mode, the sort mode is set to custom.

### 5.4 The mod card
- Size full column x 140. Surface: mod-card colour in the theme card shape (16 dp), 1 dp border, subtle hover
  glow. Never takes keyboard focus.
- Upper area (flex): the mod's preview/icon centred. If a preview composable exists it is drawn with 12/8 dp
  padding; else a 48 dp icon; else the title text (titleSmall, 18 sp line height).
- Footer: min height 36 dp, full width, centred title text (16 sp, 16 sp line height, 8 dp horizontal
  padding). Disabled mod: footer fill is text colour at alpha 0.08, text colour at 0.75 alpha. Enabled mod:
  footer fills with the accent gradient (horizontal, accent to accent-end), plus an "accent sheen"
  highlight in glass themes, and the title uses the theme accent-text colour (text colour instead when the
  footer is inset and contrast with the accent is at least 3.0). The footer cross-fades 150 ms between
  states.
- Glow: non-inset themes with shadows get a faint radial vignette (text colour, alpha 0 to 0.08 growing outward,
  radius 0.9 of the shorter side) and, at the bottom, a 50 dp high accent glow band whose strength animates
  between 0 (off) and 0.5 (on).
- Inset footer (PolyGlass themes): footer is a glass pill inset 5 dp left/right/bottom, radius concentric with
  an inset of 6, hover "pull" of 0.75 dp, no vignette.
- Footer is a switch (role) that toggles the mod if it supports toggling and is not read-only. If the mod
  needs setup and is being enabled, click opens its settings instead. Read-only toggles (owned elsewhere)
  poll their state every 250 ms and show a lock.
- Card body click opens the mod's settings page (or its custom open handler). Right-click opens the context
  menu.
- **Status dot** (top right, 11 dp inset): a 12 dp ring with a 6 dp dot, tooltip above ("can be toggled", "can't
  be toggled" or the lock reason). If toggleable: ring is an accent gradient darkened 42 percent toward
  `0xFF181600`, dot is the accent gradient, a soft 5 dp outer glow (ring colour alpha 0.41, 0.25 at 40 percent,
  0.09 at 80 percent, 0 at the edge) is drawn. If not toggleable: ring `0xFF4F505B` at 0.5 alpha with a solid
  `0xFF4F505B` dot, no glow. If locked (read-only): a 12 dp lock icon, accent when on, text colour at 0.6
  alpha when off.
- Top-left corner toggles (a Row with the card's inset): **favourite star** (shown always when favourited,
  otherwise only while the card is hovered and hover hint is enabled, as a faint empty star) and **hide eye**
  (shown while hovered, or always when the mod is hidden). Both are Large ghost icon buttons; resting emphasis
  alpha is a small hint value rising to 1 on hover or when active; hidden until hover otherwise. Star colour is
  the theme favourite colour when favourited.
- Card context menu (in order): Open settings (disabled for non-OneConfig mods without an open handler),
  Add to / Remove from favorites (star/star-filled), Hide mod / Show mod (eye-off/eye), Enable / Disable
  (check-circle/x-circle; only when toggleable; disabled with a reason when read-only; enabling a mod that
  needs setup opens settings), separator, Open HUD editor (only when the mod has HUD elements), Reset to
  defaults (danger, only for OneConfig-owned config that is not UI-only), separator, Copy mod id (detail shows
  the id), plus entries contributed by other mods.
- Reset confirm: dialog "Reset <mod>?" explaining every setting goes to its default and cannot be undone,
  confirm label "Reset settings", refresh icon. The reset walks the whole settings tree, resets keybinds
  through their own path, skips the mod's own enable switch.

### 5.5 Drag reorder
- Press and drag a card (3 px slop). The dragged card is lifted into an overlay drawn above the grid (the page
  clip is removed while dragging so it can leave the pane) with a settle-back animation; the grid auto-scrolls
  near its edges; other cards re-flow live as the dragged card moves; drops are constrained to the contiguous
  run of cards in the same group (type header boundaries are walls).
- The card's original slot shows a dashed outline: 1.5 dp stroke, dash 6 on 4 off, colour secondary text at
  0.6 alpha (favourite colour when the drop would favourite it) plus a faint 0.08 fill; outline fades in/out
  150 ms.
- Dropping above the favourite/non-favourite boundary within the group toggles favourite status: index less than
  the boundary means favourite; greater means not; equal keeps current. The favourite star on the lifted card
  previews the new state.
- Persisted order: file `mod-order`, one mod id per line (UTF-8). Reordering rewrites only the positions of the
  affected group's ids and leaves every other id in place; if an unknown mod is first dragged, the file is
  seeded from the full current displayed order.
- Keyboard alternative: Alt+Up / Alt+Down on a focused card.
- Per-profile: mod enable toggles are saved per profile (1.8.0).

### 5.6 Persisted per-mod state
Favourites and hidden ids (each a small file/list), type-collapse state, sort id, order list. Mod ids are
normalised before lookup (case/alias keys).

## 6. Per-mod settings page (the config screen)

### 6.1 Structure
- A mod's options form a tree: top-level **categories** (tabs), each with **subcategories** (labelled
  sections), each containing nodes: leaf option rows, **accordions** (collapsible groups, optionally with a
  master switch in the header), **submenus** (push a deeper route; breadcrumb in header; an inline variant
  exists). Metadata per node: title, description, icon, order index, hidden, collapsed-by-default, full-width,
  hide-label, category, subcategory, min/max/step/unit, options and labels, a visualizer override.
- Category tabs: chip-style tab row at the top (19 dp gap below). With a page search active, all categories
  list at once (no tab), and a tab switch otherwise slides like pages. Selected category is remembered per
  page, and an option jumped to selects its category.
- Entries stack vertically with 8 dp gap. Subcategory headers: small uppercase section label with 8 dp top
  padding. Category headers (in search) likewise.
- Conditional visibility: each option has a display state (Shown, Disabled, Hidden) that may change as other
  values change. Hidden = not drawn. Disabled = drawn at 0.5 alpha with interaction blocked (including
  keyboard). Group header switch off disables children; group head hidden hides the group if its head is its
  master switch.
- Row card (option): full width, card surface (16 dp radius), padding 16 horizontal and 12 vertical (0 vertical
  when nested in an accordion, with 8 dp bottom). Left: optional 32 dp icon (44 dp reserved), title (heading
  medium) and below it the description (label regular, secondary colour), 2 dp apart, 12 dp icon gap. Right:
  the control, default width **300 dp**; the label area shrinks so the control fits (reserved: icon 44, action
  button = compact height + 16, chrome 108 dp, slider spinner 96 dp). If the label would be too narrow the
  control wraps below the label (10 dp gap) at full width. "Full width" and "hide label" metadata force the
  full-width layout. Compact mode tightens padding inside grids.
- "More actions" button (three dots) at the row's right, shown only while hovering the row (preference "Show
  More Actions Button", default on); right-click anywhere on the row opens the same menu. Menu: Reset to default
  (disabled when there is no default), Copy value, Paste value (colour options use Copy hex and Paste hex),
  Unbind (keybinds only); other mods can add entries.
- Accordion/group card: header row min 32 dp icon, title, optional switch/control, chevron, padding 16/12;
  body fades and expands (spring, medium-low stiffness) with 12 dp bottom padding; expanded state is
  persisted ("collapse store"). Search auto-expands groups holding matches.
- Option width grid: accordion bodies with small controls lay out in a multi-column options grid (unverified
  column math).
- **Jump highlight** (after a jump from search): accent tint over the row with a 1.5 dp accent outline at 0.7
  alpha, ramp in 150 ms, hold 700 ms, fade out 900 ms (total ramp-out begins after the hold). Scroll uses a
  bring-into-view request; manual scrolling cancels the scroll but still plays the highlight.
- Search within a page: the header search box filters live (local, not the corpus): a node stays if its title,
  id, description, category, or subcategory matches (substring, plus fuzzy per section 11); a matching category
  or subcategory keeps everything under it; groups/submenus stay if their head or title matches or any child
  matches.
- Text markup: titles/labels accept legacy section-sign formatting codes and rich components; plain labels
  strip the codes for lists.

### 6.2 Option types (each a control with metadata)
Defaults noted where verified.
- **Switch / Checkbox** (boolean): 42 x 21 dp switch track, 3 dp thumb inset; thumb moves with a spring, track
  accent fades in over 150 ms; optional "On"/"Off"/"Mixed" state label. Checkbox size 20 dp. Listens to
  external changes (a mod's master switch flipped elsewhere updates in place).
- **Slider** (numeric): slider of width 300 dp, a value box (80 dp) with up/down steppers, gap 16 dp. Value
  clamps to min and max, snaps to step; the number field accepts typing (digits, one dot, leading minus),
  formats with up to 2 decimals unless the step has more; integer-typed options round. Arrow keys step by the
  step. Thumb size by control size: 14/19/21/24; track thickness 4/5/6/7. Sound: tick per step.
- **Number** (spinner): compact number field with unit suffix, range clamp, step (explicit step; else 1 if both
  bounds are whole numbers, else 0.1; integer types force step >= 1), format.
- **Range** (two thumbs, lower and upper) and **Number chain/list** variants exist, plus an **Inheritable
  slider** (value that can inherit from a parent).
- **Dropdown**: select field of width 300; options come from enum constants, a string list, or an int index
  (store the index, enum constant, or string depending on the property type). Label translations refresh when
  the language changes. A "Mixed" placeholder is used when a multi-selection disagrees.
- **Radio group**: horizontal radio list, same option sources.
- **Text**: single or multi-line (multi-line min height 96 dp), placeholder, optional regex; invalid text is
  flagged but only valid text is committed; leading text-input icon.
- **Colour**: swatch button that opens the colour picker popup; supports opacity, optional chroma (rainbow
  cycling with speed, default speed 1) and optional gradient (second colour) when the option allows. Hex
  input with copy/paste; saved value tracks chroma turn separately from live hue so the picker matches other
  chroma colours. Picker commits on release / enter. Colour-list variant exists.
- **Keybind**: see section 8.
- **Button**: primary text button (default label "Click"); optional confirm text turns it into an armed
  confirm button.
- **Info**: icon + heading + optional body; types Info (accent icon, info glyph), Success (check-circle),
  Warning (alert-circle), Error (x-circle); 24 dp icon, 10 dp gap.
- **File** / **File list**: read-only-ish path field with Browse button (native file dialog; placeholder text).
- **Multi-select dropdown** with Deselect all and (since 1.8.0) Select all rows.
- **List editors**: text list, number list, range list, colour list, file list, draggable list (reorderable),
  tile-picker lists (item picker, effect picker with a searchable grid: title, search by name or id, empty
  and unavailable states). List rows support Add, Insert above, Duplicate, Remove, Move up/down (Alt+Up/Alt+Down).
- **Header/section/divider** are structural nodes, not rows.
- Reset: every option captures a default value at initialisation; "Reset" restores a copy of it (colour
  resets also refresh the accent), re-creates the control (an "epoch" counter forces redraw), and can cascade to
  linked properties for composite visualizers. Options with no captured default have reset disabled.
- A mod's own master toggle is a property flagged as the group enable switch, shown on the mod card and in the
  settings header; mod toggles are persisted per profile.

### 6.3 HUD-bearing mods
Mods with HUDs show a HUD configuration screen variant (position/scale/background settings in place of generic
options) and an "Open HUD editor" entry (see section 14). Resetting HUD settings has its own flow.

## 7. Profiles page

- Top row: tab chips "All profiles" and "Favorites" (star) on the left; on the right two toggle chips: **Separate
  controls** (keyboard icon; selected = Minecraft controls are stored per profile; default on) and **Open
  folder** (folder icon; opens the profiles directory in the OS).
- Loading shows a loading empty state with retry on failure.
- Grid: **5 fixed columns**, cards **180 dp high**, in the same gutters. Cards: a "create profile" card first
  (All tab, no search) with a name field, placeholder, "Enter a name" validation, and a Create button; errors
  (duplicate or invalid name) appear inline. The first profile entry is the Default profile (empty id;
  not editable: cannot rename, clone-from-default allowed per menu, cannot delete).
- Profile card: icon (chosen from a fixed set: profiles, star, settings, cog, paintbrush, keyboard, hud,
  combat, qol, cloud), name, active state shown by a 3 dp accent border (1 dp otherwise), favourite star (shown
  when favourited, tinted favourite colour), and an options button visible while active, hovered or its menu is
  open. A per-profile **keybind field** shows when the card is active/hovered/menu-open or a bind exists; it
  switches to that profile when pressed (preference "Show Profile Switch Notification" default on; toast shows
  "Profile switched / Now using X", failure toast otherwise). Per-profile keybinds are stored in a tab-separated
  file `profile-keybinds` (modifier byte, key codes, mouse codes, profile name) and follow rename/delete.
- Click a non-active card to apply (instant, blocks the UI behind a busy overlay that appears after 400 ms with
  a "game will stop responding" note). Clicking the active card does nothing.
- Card menu: Apply (tick), Rename, Clone, Change icon, Favorite/Remove favorite (star), Export / share (zip
  with a native save dialog, filter "Zip archive"), Delete (trash). Rename and clone open an inline editor on
  the card. Delete confirms ("Delete profile?", names the profile, confirm "Delete profile"; deleting the
  active profile switches away).
- Search filters by name. Empty states: "No favorite profiles." (star) or "No profiles match" (search icon).
- Failure of any action raises an error notification "Profile action failed" with the reason; the list always
  refreshes afterwards, even if the page closed mid-operation (operations are non-cancellable).
- Storage: profile folders under `profiles/<name>`; the empty profile name means the plain `config` folder.
  Active profile and favourites and the separate-controls flag live in `oneconfig/profiles.json`.
  Config managers: internal (`./oneconfig`), core (`./config`), backup (`./oneconfig/backup`), active (current
  profile folder).

## 8. Keybinds page

### 8.1 Page layout
Single scroll column (12 dp spacing, gutter 25):
1. **Keyboard and mouse map** (1.8.0): an on-screen keyboard (keys 26 to 42 dp units, 4 dp key gap, 14 dp case
   padding) and mouse diagram. Legend: In use / Clashing / Free (7 dp LED dots). Layout selector: Auto-detect
   (shows detected layout), US QWERTY, UK QWERTY, French AZERTY, German QWERTZ, Dvorak, Colemak (220 dp
   dropdown). Hovering a key shows a tooltip card (max 360 dp): key name, whether it is unavailable to
   Minecraft, "Nothing uses this key yet", or "Used by N keybinds" listing up to a few with "and N more",
   conflict warning, and hint "Click to change it" / "Click to bind something to it". Clicking a key opens a
   panel to bind something to it (search "Search keybinds", "Hold" modifiers, "Unbind", "Close", "No keybinds
   match", "Not bound"). Single-key-only binds show a note. A selected key gets a 1.5 dp accent outline offset
   3 dp. Mouse buttons labelled Left, Right, Middle, Back button, Forward button.
2. 1 dp divider with 12 dp vertical padding.
3. **Grouped list** per mod: collapsible group header (mod icon 18 dp, mod title, chevron; collapse state
   persisted per mod id; always expanded while searching), then one row per keybind.
- Row: card surface (16 dp radius), subtle radial vignette (text colour alpha 0 to 0.05), padding 16/12.
  Left: optional 32 dp icon, title (heading medium), description, and a caption line "Category / Subcategory".
  If conflicting: a 12 dp error-coloured alert icon plus "Conflicts with A, B". Right: a More-actions button
  (on hover) and the keybind field.
- Hidden-display keybinds are skipped; disabled ones are dimmed and non-interactive.
- Empty states: "No keybinds available." (keyboard) or "No keybinds match" (search), "Searching..." while the
  first result is pending. Group headers match on mod title or id as well as entries.

### 8.2 Keybind field and capture UX
- A 32 dp field (width up to 300) showing a keyboard icon plus the bound key combination, or "None". Clicking
  enters recording: the field fills with accent at alpha 0.2, border turns accent, text turns accent and reads
  "Press keys..." until something is pressed, then shows keys joined with " + ".
- Recording collects physical keys and mouse buttons as they go down; the bind **commits when everything is
  released**. Modifier keys (Ctrl, Shift, Alt/Meta) are folded into a modifier mask; if only modifiers were
  pressed they are stored as keys instead. Escape (always) and Backspace/Delete (when nothing recorded yet)
  clears the bind and ends recording. Typed-character events are swallowed. "Single key" binds commit on the
  first key or mouse button press. Text-input focus is held while recording so global shortcuts do not fire;
  the menu key is suppressed.
- Hover (enabled): fill alpha x0.8, border text colour at 0.2. Conflict: border and text in the error colour
  (not while recording). Conflict is automatic by comparing against all other binds unless the caller passes it.
- Conflict map: any two bound entries whose combinations conflict are linked both ways; entries with no key
  and no mouse button are ignored; **context-gated binds are exempt** (Minecraft debug keys, spectator keys and
  the spectator shader toggle never conflict).
- Writes keep the runtime keybind subtype, update the global keybind manager and bump a conflict revision.
  Reset restores the captured default (clearing to none if the default is empty; or null for nullable
  binds). Unbind clears.

## 9. Themes page

- Layout: page-wide, then a card grid of theme families, **3 columns, 20 dp gaps**, followed by (when a custom
  theme is active) the custom theme editor, then a no-backdrop-glass notice when the platform cannot do frosted
  glass, then the Sound Theme picker. The page is also a normal settings page (the animation/transparency
  options of 6 follow below the header).
- **Family card**: surface in mod-card colour with the window corner shape, border 1 dp (selected: 3 dp accent
  border, colour animates with a spring), hover glow; 12 dp padding. Top: a preview image **112 x 72 dp**, then
  title (15 sp medium) with a hover-revealed 22 dp "Make a custom copy" button (copy-plus), then the controls
  (flow layout, 6 dp gaps). Footer (min 18 dp, 12 dp above): a one-line caption (12 sp secondary). Clicking
  selects the theme (activation sound chosen by the registry); right-click menu: Activate (disabled when
  active), Make a custom copy (disabled with a "limit reached" reason at the cap).
- Controls: a Dark/Light **icon segment** (moon/sun, compact segmented tabs with tooltips) when the family has
  two variants; for glass themes also a **Tint** segment (Clear = layers, Stained = paintbrush, Biome = cloud;
  stored as an int index, default Clear); for palette families with more than two variants a compact dropdown
  of variants (max 150 dp) plus an **accent swatch row** (16 dp circles, 3 dp apart; selected gets a 2 dp text
  colour ring and 3 dp inset, others 2 dp inset; last swatch "Use my accent", a profiles glyph on the component
  colour; tooltips show swatch names).
- Captions: Polyfrosted "frosted panels, the OneConfig look", PolyGlass "see-through glass over your game",
  Minecraft "blocky, like the vanilla menus", Custom "make your own from any theme".
- Final card = **Custom** (adds a new theme from the active/first base).
- **Built-in themes** (the registry's non-custom entries): Polyfrosted Dark/Light, PolyGlass Dark/Light,
  Minecraft Dark/Light, and palette families Catppuccin (Latte, Frappe, Macchiato, Mocha; 14 selectable accents
  Rosewater, Flamingo, Pink, Mauve (default, index 3), Red, Maroon, Peach, Yellow, Green, Teal, Sky, Sapphire,
  Blue, Lavender) and Nord (Nord, Nord Light). Default active theme: **Polyfrosted Dark**. (Catppuccin and Nord
  hex values were not transcribed; use the public palettes.)
- **Accent**: player accent colour option (`Accent Color`, gradient capable, chroma capable, default
  `0xFF2B4BFF`). Accent drives the nav selection, footer fill, switch tracks, focus ring, glow, jump highlight.
  A second gradient colour gives a horizontal two-stop accent brush; chroma accents recompute every frame (no
  gradient while chroma is on). Palette themes override the accent with the chosen palette accent (flat colour,
  no gradient); "Use my accent" falls back to the player accent. The accent-text colour is picked per theme to
  keep contrast against the current accent. State colours (error, warning, success, info) are adjusted for
  contrast against the page.
- Accent animation: colours derived from the accent tween from their previous value only when a key changes
  (e.g. selected state), otherwise follow the live accent so chroma never lags.
- Theme changes crossfade (animated theme interpolation).
- **Sound Theme** picker below: heading plus cards (min 240 dp wide, as many columns as fit, max 3): Match Theme
  (caption "follows your theme, now X"), OneConfig (soft modern sounds), Minecraft (classic sounds). Selecting a
  different one plays an open-sound preview; re-clicking the current one plays the click sound. Stored as an
  int (0 match, 1 OneConfig, 2 Minecraft; default match).
- **Custom themes** (max **8**; names limited to 24 characters): editor with Name (placeholder "Theme name"),
  Base (Polyfrosted, PolyGlass, Minecraft), Colours (Background, Surface, Text, Secondary text, Border), a
  contrast checker with a Fix button for text and secondary text ("hard to read on this background"), Accent
  (with "Use my accent"), Corners (Rounded, Soft, Square), Glass options, and actions Copy code, Paste code,
  Duplicate, Reset to base, Delete (click-again confirm plus a confirm dialog). Share code format: prefix
  `OC1.`, max 1024 characters, JSON payload limit 4096 bytes (encoded; encoding not verified). Importing asks to
  add "X" with Add/Cancel; invalid codes say "isn't a theme code"; clipboard failures have their own message;
  duplicates are named "X copy" or "X copy #N". Stored in `custom-themes.json` (plus a `selected` key); import
  beyond the limit is kept for later.
- Theme flags: glowless (PolyGlass), inset mod footer (PolyGlass), translucent selection (PolyGlass),
  liquid-glass rendering (PolyGlass), cursor effects (PolyGlass), pixel/Minecraft style (Minecraft), shadow on/off.
  `ThemeStyle` is Flat, Glass or Pixel; pixel themes square all shapes, swap icons and fonts.

### 9.1 Colour tokens (built-in themes)
Order: page, sidebar, chip, modCard, component, popup, border, text, secondary text, accent-text, shadow,
control thumb, favourite.
| theme | page | sidebar | chip | card | component | popup | border | text | text2 | accentText | shadow | thumb | fav |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Polyfrosted Dark | C411171C | B3151C22 | B2232D32 | 59232D32 | FF1A2229 | FF1A2229 | 1AFFFFFF | FFD5DBFF | FF757883 | FFFFFFFF | FF000000 | FFFFFFFF | FFFFD700 |
| Polyfrosted Light | B0D0D7F3 | B2D0D7F3 | 59EBF2FF | 59EBF2FF | B2EBF2FF | FFD0D7F3 | 4DA3A3A3 | FF000000 | FF4E5661 | FFD5DBFF | FF000000 | FF1A2229 | FFB57D00 |
| Minecraft Dark | FF11171C | FF11171C | B2232D32 | 59232D32 | FF1A2229 | FF1A2229 | 1AFFFFFF | FFD5DBFF | FF757883 | FFFFFFFF | FF000000 | FFFFFFFF | FFFFD700 |
| Minecraft Light | FFE8EDFF | FFE8EDFF | 59EBF2FF | 59EBF2FF | B2EBF2FF | FFD0D7F3 | 4DA3A3A3 | FF000000 | FF4E5661 | FFD5DBFF | FF000000 | FF1A2229 | FFB57D00 |
| PolyGlass Dark | 8C16181D | 99121419 | 7322252C | 6620232A | 7322252C | CC16181D | 14FFFFFF | FFFFFFFF | B3FFFFFF | FFFFFFFF | FF000000 | FFFFFFFF | FFFFD60A |
| PolyGlass Light | A6FAFBFF | B8F2F4F9 | 66FFFFFF | 57FFFFFF | 66FFFFFF | D1F6F8FC | 8CFFFFFF | FF0E1118 | 9E0E1118 | FFFFFFFF | FF1A2440 | FFFFFFFF | FFC98A00 |
Notes: default control track colour `0xFF74777F`; Minecraft themes have shadows off and every radius 0; flat and
glass themes use radii bg=20 (LG), nav entry=8 (SM), card=16 (MD), checkbox=6 (XS), button=8 (SM), popup=16
(MD), circle=full. Minecraft themes use Minecraft-style fonts (Regular, Bold only) and an all-zero radii set;
flat themes use Poppins.
The Polyfrosted pair, PolyGlass pair and Minecraft pair each use their own wordmark asset (dark, light, white,
pixel variants). Theme preview thumbnails are SVGs sized 112 x 72.
Light flag: derived from contrast unless a theme sets it explicitly.

## 10. Preferences page

The page is a standard settings page of the config named "OneConfig" with sub-sections. Defaults (type;
range; default). Option titles are the identifiers.

GUI: OneConfig Keybind (Right Shift); Quick Search Keybind (Backslash); Keybind Closes Menu (off); Pause Game
(off); Remember Last Page (dropdown: Never, 15 seconds, 1 minute, 5 minutes, Always; default 15 seconds);
Reopen HUD Editor (off); Show More Actions Button (on); UI Scale (50 to 200, step 5, 100); Reduce Transparency
(off).

Search: Instant Search (on); Animate Search Results (off, experimental: fades/scales/slides results in);
Search Keybind (Ctrl or Cmd + F); Search Typo Tolerance (0 to 10, default 2; 0 = exact substring only).

HUD Editor (all keybinds use Ctrl, or Cmd on macOS, as the action modifier unless noted): Open HUD Settings
(E), Show/Hide HUD (H), Lock/Unlock HUD (L), Copy HUD (C), Cut HUD (X), Paste HUD (V), Duplicate HUD (D), Reset
HUD Settings (R), Delete HUD (Delete, no modifier), Select All HUDs (A), Undo (Z), Redo (Shift+Z), Redo
alternate (Y), HUD Editor Shortcuts (F1, no modifier), shortcuts alternate (Slash with action modifier). Toggles
and sliders: Show Keybind Hints (on; shows shortcut next to context-menu items), Selection Box Animation
(on), Selection Box Animation Duration (50 to 500 ms, step 10, 150), HUD Drag Opacity (0 to 100, step 5, 60;
how visible the menu stays while dragging a HUD).

Appearance (theme config, also on the Themes page): Animations (master switch, on); Reduced Motion (off);
Opening Animation (on); Closing Animation (on); Opening Time (0.05 to 2, step 0.05, 0.6 s); Show Page
Animations (on); Page Animation Duration (0.1 to 0.6, step 0.05, 0.3 s); Show Opening Page Animation (off);
Pixel Grid (Automatic/Always/Never, Automatic); Page Opacity (0 to 100, 88); Sidebar Opacity (0 to 100, 80);
Background Glow Opacity (0 to 100, 25); Background Blur (on). Animation options hide when Animations is off;
transparency options hide when Reduce Transparency is on. Hidden stored state: active theme name, sound theme,
glass tint, palette accents, migration flags.

Sounds: UI Sounds master (on); Menu Sounds (on); Click Sounds (on); Slider Sounds (on); HUD Editor Sounds (on);
UI Ambience (on); Lower Music During Ambience (on); UI Sound Volume (0 to 100, step 5, 100); UI Ambience Volume
(0 to 100, step 5, 100).

General: Show First Launch Message (on); Show Profile Switch Notification (on).

Other stored flags: editor tour seen, opening-behaviour legacy values, percent migration. First launch shows a
welcome notification telling the player which key opens OneConfig.

## 11. Search

- Header search box (256 dp): a magnifying-glass field with a clear button; placeholder is per page ("Search
  Mods...", "Search Profiles...", etc.; hidden while focused on the global form). Text persists per history
  entry. Pressing the Search keybind (Ctrl/Cmd+F) focuses it (with select-all when in minimal mode).
- Scopes: Mods (cards), Options (every option in global search), Keybinds, HUDs (editor library), and a
  per-config scope (options inside one mod). Documents have id, scopes, metadata (title, id, description,
  section, category, subcategory, tags, mod title/description, path) and a payload.
- Matching rule (default provider): case-insensitive substring; otherwise, if typo tolerance > 0, edit
  distance (Levenshtein, early-exit at the cap) against the whole string, then against each word split at
  space, dot, underscore, hyphen or slash, then against camelCase pieces (piece tolerance = min(tolerance,
  length/4)). Mod cards match on title and tags only. Options match title, description, tags, plus category/
  subcategory/id in config scope; keybinds also match path; HUDs also match mod title.
- The corpus is built in the background, debounced 250 ms after config registrations, incrementally per
  source (configs, keybinds, HUD providers, HUD mod cards). Other mods can register providers by priority and
  sources; failures fall back to the default provider.
- Global results screen (shown on the Mods page when the query is non-blank): groups results by mod with a
  "MODS" header; shows "Searching...", "No results for ...", or a mod that has "No settings available." /
  "No settings match". Results are option rows (clicking jumps to the option, see 2). Option "Animate Search
  Results" adds an entrance animation (off by default).
- Other pages' local searches: Profiles filter by name; Keybinds search via the corpus (plus group header
  match); Changelog matches version text; Credits matches name+role; Themes/Preferences filter their settings.

## 12. Sounds and ambience

- Event types: OPEN, CLOSE, CLICK, SLIDER_TICK, HUD_SELECT, HUD_DRAG_START, HUD_DRAG_END, HUD_RESIZE_START,
  HUD_RESIZE_END. Enable gating: Menu Sounds for open/close, Click Sounds for click, Slider Sounds for slider
  tick, HUD Editor Sounds for all HUD events; all require UI Sounds. Master volume = UI Sound Volume/100.
- Two sound themes. Modern (OneConfig): custom samples `ui.open`, `ui.close`, `ui.click`, `ui.slider`; HUD
  select uses click at pitch 1.0, drag start 0.9, drag end 1.1, resize end 1.0 (resize start unmapped in the
  modern path I read). Minecraft: vanilla UI button click at volume 0.5 for click and 0.3 for HUD select/drag
  end/resize end; open/close reuse the modern samples; slider ticks play a bass note-block note cycling
  through a pitch table (a counter modulo the table length) at volume 0.3.
- Theme resolution: Match Theme follows the active theme's pixel flag; explicit OneConfig/Minecraft override.
- Ambience: a soft loop while the menu or HUD editor is open, volume = UI Ambience Volume x 0.5 (modern) or
  x 0.08 (Minecraft); fades in unless paused; fades out on close; restarts (crossfade) when the sound theme
  changes or the active theme's resolved sound theme changes. "Lower Music During Ambience" ducks Minecraft
  music while it plays.

## 13. Notifications

- Bell panel in the sidebar footer: width 368 dp, content padding 14/16, list max height 420 dp with fade
  edges, 16 dp row gaps. Rows: 32 dp icon box (24 dp glyph; type icon or custom), title (20 sp line height),
  unread dot (accent), message (21 sp line height), optional progress bar (live-updating, 8 dp above), optional
  action buttons (16 dp apart). Row menu: Dismiss, Copy text. Footer has "Clear all notifications" with a
  trash icon. Empty state: "You're all caught up." Opening marks all read on close.
- Types: info, success, warning, error with accent colours from the theme state colours; toasts use the same
  types (default 4000 ms, max 3 visible, persistent when duration <= 0, hover pauses remaining time, a dismiss
  control). Notifications can raise an error from any page (e.g. profile action failures).
- Time labels use relative text ("just now", "N minutes ago", "Yesterday, HH:MM", etc.) and short count and
  size formats (1.2K, 3 MB).

## 14. HUD editor ("Edit HUD")

(Heavily featured; implemented in a ~4,700-line screen. This inventory covers the observable behaviour.)

### 14.1 Entry and chrome
- Opened from the sidebar "Edit HUD", from a mod card's "Open HUD editor", from the OneConfig key (when the
  menu is open and "Keybind Closes Menu" is off), or from minimal mode. The menu shell dims to the "HUD Drag
  Opacity" preference while a HUD is being dragged and returns afterwards.
- First open shows a guided **Editor Tour** (steps: welcome, adding, dragging/resizing, merging (with an
  interactive task), anchoring, settings, style, fonts, layout, done) with Next/Back/Skip/Done and re-launch
  via right-click empty space "Editor Tour"; seen-flag persisted. Buttons: Next, Back, Skip tour, Done.
- A "Keyboard shortcuts" pill is shown at the bottom with the bound key; opens a shortcuts sheet (see 14.7).
- The editor can show outlines for vanilla HUD elements (boss bar, scoreboard, action bar, titles, etc. since
  1.8.0), toggled with V (hold-style while pressed). The grid toggles with G. These keys are ignored while text
  is being typed.

### 14.2 HUD library panel
- Title "HUDs", search "Search HUDs...", hint "Click to add, drag to place it yourself". Grid of cards with
  rendered previews (preview scale 2x, card padding 12, label height 20, preview max card height 160,
  stretch tolerance up to 1.35x, row height tolerance 0.6, grid gap 12), grouped by mod with an "Other"
  group; empty states "No HUDs found" and "No HUDs installed" (with a call to action to install a HUD mod).
  Card actions: Add to screen, Open mod settings. Clicking a card drops the HUD at the screen centre;
  dragging places it.
- A "Legacy HUD" label is applied to old-style HUDs.

### 14.3 Selection, move, resize
- Click selects; Shift+click adds; Ctrl/Cmd+click toggles one; drag on empty space draws a marquee
  (selection box fill alpha 0.15, optional fade animation 50 to 500 ms); Select-all selects unlocked HUDs;
  Escape clears selection (then closes the panel, then steps out of a group, then cancels anchoring).
- Drag slop 3 px; double-click window 400 ms. HUD box outlines: idle alpha 0.2, hovered 0.5, hidden 0.4,
  group member 0.45, entered-group 0.35; the dragged HUD is filled at alpha 0.10. Selection handles: corner
  handles of radius 3.5 px (1.5 px stroke, accent) with a 4 px hit reach; a floating action bar above the
  selection shows 3 action icons (lock, visibility, settings) plus a size badge (16 px high, 5 px padding, 10 px
  font, 9 px gap).
- Locked HUDs cannot be moved or resized (a toast "HUD element locked" explains how to unlock). Hidden HUDs
  stay editable.
- Resize: drag a corner scales uniformly; Shift+drag corner resizes width and height freely (for resizable
  box HUDs). A minimum of 12 px of the HUD must stay visible on screen.
- Move: arrow keys nudge 1 px; Shift+arrows 10 px; nudges settle (commit to history) after 300 ms.
- Snapping: hold Alt to disable. Snap distance **6 px** (scaled by UI scale). Snap lines come from the screen
  edges, centre, a 4 px screen padding, and every other HUD's left/centre/right (top/centre/bottom).
  Additionally equal-gap snapping: the dragged HUD snaps to repeat existing gaps between obstacles or to the
  midpoint of the free space between neighbours. Guides draw as lines between spans; gap markers show sizes.
  Grid size 8 px (grid lines alpha 0.07, toggle G). Merge-touch tolerance 2 px.
- Alt+hover measures the gap to another HUD (line alpha 0.45, badge alpha 0.9, badge radius 3 dp, 2 px end
  dots, tolerance 0.05).
- Merging: HUDs stacked flush above/below or side by side merge their backgrounds into one shape. Merge needs
  the background on, no border, same colour unless "Merge with different colours", edges aligned unless
  diagonal merging; hints explain each reason it won't merge ("won't merge because..."). Dropping shows "Let go
  to merge". Ctrl/Cmd+drag pulls a HUD out of a merged group. A merge link outranks stored position.
- Anchoring: context menu Anchor then click a point on the HUD (9 anchor points; idle dots 2.5 px alpha 0.35,
  active 5 px, hit radius 10 px), then click a point on another HUD; right click removes; Esc cancels/steps
  back. Anchored HUDs derive their position (relativeX/relativeY are ignored by undo while anchored).
- Groups (1.9.1): group HUDs so they move, scale and lock together; a locked HUD inside a group stays put in
  the group but follows it; double-click enters a group to edit members; Esc steps out. Menu: Group, Ungroup, Add
  to group, Remove/Detach from group. Selecting several HUDs or a group shows common settings in the panel and
  edits apply to all.
- Context menu: Settings, Show/Hide, Lock/Unlock, Delete HUD, Duplicate, Anchor/Remove anchor, Group
  options, Undo, Redo, Reset position, Reset HUD settings, Allow merging, and the Editor Tour; each item can show
  its shortcut when "Show Keybind Hints" is on. Align commands: left, centre, right, top, middle, bottom.
- Clipboard: Copy, Cut, Paste (at the pointer), Duplicate; clipboard format is internal.
- Undo/redo: whole-layout snapshots of every active HUD's option values plus its own hidden/locked/position/
  scale; limit **100** entries; float equality tolerance 0.001; ignores layout-derived options; undo restores
  deleted HUDs and removes added ones; restoring a position drops merge links first.
- Sounds: select, drag start/end, resize start/end (see 12).

### 14.4 HUD settings panel (right dock)
Three tabs: **Settings** (the HUD's own options, show-in conditions), **Style**, **Layout**; panel gutter 16 dp;
search "Search settings..." within the panel; a Reset card per section. Style sections: Text (font Minecraft or
Poppins, size, weight, case, bold/italic/underline modifiers, colour, shadow normal/full, shadow colour),
Background (colour, radius, merge with neighbours Off/Normal/Diagonal, merge with different colours, keep
hidden), Border (size, colour). Layout sections: Position & Scale (position, lock position, scale, follow GUI
scale, reset to follow), Size & Alignment (static size, width, height, anchor, content alignment with 9 cells:
top-left through bottom-right), Padding (horizontal/vertical or per side), Visibility (show in F3 screen, tab
list, GUIs, chat). The anchor control's hint says it pins the corner and the HUD grows away.

### 14.5 Reset behaviour
Reset HUD Settings restores option defaults for the selection; Reset position restores the default position
and scale. HUDs wrapping another mod's element keep hidden/locked/position/scale in their own state, saved
and restored by undo.

### 14.6 Persistence
HUD placement stored as a section, relative offsets and a custom scale per HUD, plus the layout reference size
and a position schema id, all excluded from undo. Locks and visibility persist.

### 14.7 Shortcut sheet
Groups Moving, Selecting, Editing, listing: arrows nudge 1, Shift+arrows 10, Alt+drag no snap, Ctrl/Cmd+drag
detach, Shift+corner free resize, G grid, V vanilla outlines; marquee, Shift+click add, Ctrl/Cmd+click toggle,
Alt+hover measure, select-all, Esc deselect; settings, show/hide, lock, copy, cut, paste, duplicate, reset
position, delete, undo, redo, show-shortcuts (rows only appear for bound keybinds).

## 15. Changelog page (about menu)

- Master/detail: left version list 200 dp wide (no scrollbar, fade edges), right release detail with
  gutter 25. List groups versions by series (the first two dot/dash-separated numeric parts) with series
  headers; each entry is a nav-style row with the version and, on the right in caption text, a short date;
  the installed version has an accent status dot; the list auto-scrolls so the selected/installed version
  appears a few items from the top. Initial selection: previously chosen for this source, else installed
  version, else newest.
- Detail: sections by change kind (Added, Changed, Deprecated, Removed, Fixed, Security) and a separate
  "For mod developers" block; entries are rich text with clickable links; links to Modrinth versions open an
  in-app dialog (loading, failed with retry, empty message, "Open on Modrinth"); other http(s) links open the
  browser. Release date is formatted with the locale.
- Search matches the release text (fuzzy). Empty states: "No changelog was bundled" (none), "No releases match",
  loading state; other mods can register changelog sources (Modrinth-backed or bundled); the active source is
  selectable via the host.
- A changelog dialog also pops after updates (separate dialog screen; trigger not verified).

## 16. Credits page (about menu)

Scrollable page: masthead (hidden while searching), "People" grid of contributor cards with 3D-style player
heads (head fetched by UUID; offline UUID derived from the name when none is known) and a role caption,
"Special thanks" with a second people grid and three info cards (the Polyfrost team, translators, contributors
and testers). Translators card lists mod and resource-pack translation sources discovered at runtime (mods
with translation namespaces, packs with authors). A legal and corporate section exists. Search filters people
and cards by name+role. A role string set exists per person; names/roles are content, not behaviour.

## 17. Data model

- Settings are a tree of nodes (trees and properties). Property: id, type, value, default (captured copy at
  init), title, description, metadata map, visualizer override, display state (Shown, Disabled, Hidden) with
  listeners, callbacks fired on change (returning true vetoes). Metadata keys: title, description, category,
  subcategory, min, max, step, unit, options, optionLabels, visualizer, collapsed, icon, index (order), hidden,
  fullWidth, hideLabel; plus custom keys (default, defaultNull, singleKey, resetWith, controlWidth, swatch,
  ui_only, open_original_screen).
- Mods register configs from annotation-style declarations (Switch, Slider, Dropdown, Keybind, Color, Text,
  Number, Button, Info, Radio, etc., each with title, description, category, subcategory, icon, min/max/step),
  from fluent builders, or from compat collectors that translate other config libraries (Cloth, YACL,
  MoulConfig, Skyblocker) into trees. A tree builder supports groups with an enable toggle, dependencies that
  re-evaluate, share codes and per-profile migrations. Every config has id, display title, authors, credits,
  version, icon, category, and an optional toggle.
- Storage: structured text files (a NightConfig-based serializer family) per config id (e.g. `themes.json`,
  `oneconfig.json`, `profiles.json`) in the config folders listed in section 7. Small line-oriented files for UI
  state: `mod-order`, `mod-sort`, `profile-keybinds`, plus favourites/hidden/collapse stores and
  `custom-themes.json`. Writes go through a file backend with a backup folder. Configs save on menu close and on
  profile change; a failed save is surfaced via a last-failure query and a notification.
- Keybinds persist as key codes, mouse button codes and a modifier byte; mouse side buttons use mouse codes.
- Profiles switch the active config folder; "profile-specific controls" decides whether Minecraft's own keybinds
  move with the profile.

## 18. Behaviour list for a Rust re-implementation (prioritised)

1. Fixed 1391 x 700 logical canvas with fit-scale and 0.9 margin; sidebar 264; content gutters 25/19.
2. Theme token set (section 9.1), radii, type ramp, spacing, accent (solid or two-stop gradient) and overlays.
3. Page registry, history stacks, per-page tab/scroll/search memory, slide transition (1/5 width).
4. Mods grid (4 col, 140 dp cards), chips with dynamic visibility, favourite/hide, status dot, footer toggle,
   context menu, sort modes, drag reorder with group walls and favourite boundary, persisted order file.
5. Settings screen renderer for all option types, conditional display, reset, search filter, jump highlight.
6. Profiles (5 col grid, 180 dp), keybinds page with capture and conflict map, themes page with custom themes.
7. Preferences with every default listed in section 10.
8. Search corpus with fuzzy matching and scoped documents.
9. Notifications panel and toasts; sound events and ambience.
10. HUD editor (snapping, merging, anchoring, groups, undo 100, shortcuts) as a later milestone.

## 19. Open questions / unverified

- Exact OneConfig version bundled with OneClient 2.11.0; whether Quick Search (1.8.0), keyboard map (1.8.0), HUD
  groups (1.9.1) and PolyCompose rendering (1.9.0) exist in that build. Behaviour in older 1.6.x builds (for
  example the original Compose-Multiplatform UI) may differ in widths and animations.
- No Screenshots or Wardrobe pages found; check PolyPlus/launcher separately if they are required.
- Catppuccin and Nord hex values (only names and defaults captured), the "Stained" and "Biome" glass tint
  behaviour, liquid-glass shader details, cursor effects, the custom-theme share code encoding.
- Exact chip, segmented-tab, nav-item and card hover glow/pull visuals (only partial numbers read); mod-card
  type list contents for third-party mods; the lookup table that assigns categories to third-party mods.
- Option grid column logic inside accordions; exact widths of the colour picker popup; slider value-box
  behaviour on blur; behaviour of range/number-chain/inheritable-slider controls.
- Whether the resize-start sound exists in the modern theme; the Minecraft slider pitch table values; ambience
  asset details and fade timings (tick counts not captured).
- Notification toast stacking positions, animation timings, and the changelog dialog trigger.
- HUD editor: precise panel widths, action bar placement rules, grouping edge cases, clipboard format, tour
  step geometry, vanilla-outline measurement per version.
- Screen-reader/narrator behaviour beyond "every control announces role and label".
- Window-resizing behaviour while the menu is open (assumed re-fit using the same scale rule).
- Third-party-licence status of Poppins and the pixel font: confirm before bundling.
