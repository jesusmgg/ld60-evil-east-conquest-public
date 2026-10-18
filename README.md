Evil East Conquest
===

Ludum Dare 60 Compo game.

- Design your soldiers and war machines.
- Deploy your army.
- Collect resources, steal technology.
- March to the east and conquer everything along the way.

## Building
The game requires only the standard Rust toolchain with the `wasm32-unknown-unknown` target installed.

```bash
# Produces a debug build at `target/wasm32-unknown-unknown/debug`
cargo build

 # Produces a production build at `target/wasm32-unknown-unknown/release`
cargo build --release
```

## Controls

- Left click - select a unit; click empty ground to deselect
- Right click - move order for the selected unit, or attack the enemy clicked
- Shift + right click - queue a move order
- Left click a gate - move the army through it, once all of it waits in that gate's transfer area
- Design, Train, Saves, Help and Options - the buttons at the top
- Sell - bottom left, for your selected soldier or machine
- Hover a unit - its stats
- `Escape` - cancel placing a unit or a confirmation dialog, or close the open panel

Only your own units take orders. Any unit can be selected and inspected.

Debug builds only:

- `Z` - undo a turn
- Arrow keys and the minimap - move the view to another sector
- `F1` - level editor (`H` in the editor lists its own keys)
- `B` - build a WallWood on the facing tile, owned by the selected unit's player
- `Tab` - debug UI, including the selected unit's path and queued goals
- `` ` `` - debug log, `L` - toggle lighting
