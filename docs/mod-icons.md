# Mod card icons

Every mod has a card on the Mods page, and every card has an icon. This page says which mods need a rendered
preview and how a new mod gets its icon. The single source of truth is the table `KINDS` in
`crates/vantage/src/mod_icons.rs`; a test fails until a new mod has an entry there.

## The three kinds

| Kind | What the card shows | Which mods |
| --- | --- | --- |
| `Preview` | A small render of the mod's own output, drawn by `crates/vantage/src/gui/previews.rs` | Every mod in the HUD category (category `0` in `MODS`) and the crosshair |
| `Art(..)` | An icon converted from the OneClient-style set (`crates/vantage-gui/src/icon_art.rs`) | A mod that has no output of its own on screen but matches an icon in the set, such as Fullbright (sun) or Time changer (moon) |
| `Glyph` | The mod's glyph of the icon font | Everything else, until it gets a converted icon |

## Deciding for a new mod

1. **Does the mod put something on the screen that has a look of its own?** A counter, a readout, a box of text, a
   grid of slots, a crosshair. Then it needs a rendered preview: mark it `Kind::Preview`, add a `Look` for its id
   in `previews.rs::look` and show what a player would see (a short sample of its text, or its shape).
2. **Is it a world overlay, a gameplay tweak or a toggle with nothing to look at?** It does not get a preview.
   Pick an `Art(..)` icon from the converted set if one means the same thing, otherwise leave it as `Glyph`.
3. Add the mod to `KINDS` in the same position as in `MODS` (the test checks the order).

The tests in `mod_icons.rs` and `previews.rs` enforce the rule: every mod has exactly one entry, only HUD mods and
the crosshair are previews, and a mod is a preview exactly when `previews.rs` knows how to draw it. A mod in
the HUD category cannot be added without a preview, and a preview cannot exist for a mod that is not marked as one.

## Drawing a preview

A preview is drawn in the card's icon area with the menu's drawing primitives, in the menu's colours and font, so
it stays sharp at any size and needs no image file. Keep it to a few elements: the mod's sample text in a dark
see-through box, or its shape. It is a picture of the mod, not a working copy of it, so use fixed sample values.

## What not to do

- Do not add image files for previews: the menu draws no pictures, and a fixed picture would drift from the mod.
- Do not copy OneClient's or OneConfig's artwork into a preview. The converted set is the only place that artwork
  lives, and previews are our own drawings.
