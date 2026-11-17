# Native Vantage: port status

Vantage runs natively in the client (`crates/vantage` holds its code; `app/src/modding/vantage/`
feeds it the client's state). Its menu is generated from the same code as the old mod, so every
card has the same options. What differs is whether the client answers the host calls an option
needs. Calls that are not implemented refuse, which Vantage treats as a missing permission.

## Working natively

HUD elements: Keystrokes, CPS, FPS, Coordinates, Direction, Chunk info, Slime chunk, Armor status,
Potion status, Ping, Momentum, Stopwatch, Playtime, Clock, Day counter, View angles, Saturation
(with its hunger-bar overlay), Reach (without the outline), Combo, Potion counter, Totem counter,
Item counter, Inventory HUD, Array list, Scoreboard, Target HUD, PvP Info.

World drawing: Chunk borders, Subchunks, Slime chunks, Light spots, Hitboxes, the Reach highlight,
the Damage indicator and Waypoint beams and labels.

Mechanics and visuals: Block outline (the client's own outline is dropped while it is on), Zoom, Crosshair, Fullbright, Time changer (also dims a pack's sky cube),
Weather changer, Toggle sprint, Toggle sneak, Always sprint, Nametags, Hit color, Hurt camera,
Perspective (Freelook, Snaplook, look behind, key writes), Auto Text Hotkey, Hive Requeue (chat
reads and commands).

## Not working yet, and what each needs

| Card or option | Needs |
| --- | --- |
| Guardians, Blaze spawners, WAILA, Height limit, Markers, Glass | terrain reads and surface rules are ported: to be tested in game |
| Glass | `terrain::set_surface_rules` |
| CPS limiter, No swing sounds, No cam reset, Audio subtitles | wired in `app/src/modding/vantage/handlers.rs` (teleport, sound pump, action filter): to be tested in game. The limiter filters only while no mod owns the action filter |
| Item delay fix | wired (`item_use::pulse_use` -> `NativeUsePulse` -> action filter repeat): to be tested in game |
| Center cursor | wired (`read_container_screen`, `position_pointer` -> `pointer::Pending` with a native owner): to be tested in game |
| Keystrokes key labels from your bindings | works through `read_bindings`; check in game |
| Touch Menu button and touch buttons | wired (`NativeTouch` carries buttons and taps between Vantage's frame and the touch presentation): to be tested on a touch screen |

## Known behaviour

- Time, weather, hurt tilt, nametags, fullbright, movement modes and held intents stay until
  Vantage changes them. Zoom scale, look-behind and the Snaplook perspective are set every frame.
- A client system that rewrites a value every frame cancels a native setting unless it also reads
  native Vantage's request (fullbright did; `NativeFullbright` fixes it).
- The default `Keystrokes` and `CPS` elements are on; the rest start off, as before.
