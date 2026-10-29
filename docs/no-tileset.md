# A level without a tileset

The level header names three tilesets, three backgrounds and three
foregrounds, a graphics set of one of each for each of its three slots. Any of
them can be 0xFFFF, which the editor calls none. All of them can be none
except one: the tileset of the first set, which is the level's tileset. This
file explains why, and what the editor does about it.

The addresses below are from the Ghidra database these notes were read from.
They do not all match the retail US ROM (`A2DE`) that
[tileset-memory.md](tileset-memory.md) names:
`StageLayout::loadCharacterGraphics` is `020bcfc4` here and `020bcfec` there.

## Where the number goes

`StageLayout::onCreate` reads the halfword at header offset 0x0C, the first of
the three tilesets, and passes it to `SetTilesetId @ 020b02ac`, which stores it
in the global `TilesetID @ 020cacbc`. The two other tilesets are never read
into it. Neither are the backgrounds and foregrounds: those are read from the
header and the layer blocks wherever they are needed.

## Loading checks for none

Every loader of a tileset's files tests for 0xFFFF before it indexes its
table:

| Function | Loads | Check |
| --- | --- | --- |
| `StageLayout::loadCharacterGraphics @ 020bcfc4` | the tileset's tiles, and the background's and foreground's | `TilesetID != 0xffff`, and the same for `background[0]` and `foreground[0]` |
| `StageLayout::loadPalettes @ 020bcbe8` | each layer's colours, from its first block | `plttID != 0xffff` for each |
| `StageLayout::loadTilemaps @ 020bc884` | the background and foreground screens | `scrID != 0xffff` and `scrollSpeedY != 0xffff` |
| `StageLayout::LoadTileBehaviors @ 020af96c` | the tileset's behaviours | `TilesetID != 0xffff` |

Each layer is checked on its own, so a level without a tileset still loads its
background and foreground. A table entry of 0x7A5, the game's file for nothing,
is skipped the same way.

## Animating does not

`BGActor_animateTiles @ 020b67a8` runs every frame. It runs three tables of
animation functions, each entry eight bytes, a function and a parameter:

```c
tilesetID = HANDLER->tilesetData[player].id;          // the background's
if (tilesetID != 0xffff && FG_Animations[tilesetID].func != NULL) { ... }

uVar2 = TilesetID;                                    // no check
if (TilesetAnimationFunctions[uVar2].func != NULL) {
    (*TilesetAnimationFunctions[uVar2].func)(...);
}

uVar2 = HANDLER->field31_0x4b0[player - 1].field4_0x8; // the foreground's
if (uVar2 != 0xffff && BG_Animations[uVar2].func != NULL) { ... }
```

(The game names its tables the other way round from the level format, as
[lua-backgrounds.md](lua-backgrounds.md) explains: `FG_Animations` is indexed
by the background, and `BG_Animations` by the foreground.)

The background's and foreground's lookups test for 0xFFFF. The tileset's does
not. With `TilesetID` at 0xFFFF it reads
`TilesetAnimationFunctions @ 020cbb00` + 0xFFFF × 8, which is `0214baf8`: half
a megabyte past a table of 76 entries, in the middle of whatever overlay is
loaded there. Whatever word is there is called as a function every frame,
and an odd parameter word makes it a pointer to follow first. That is not a
level without a tileset; it is a jump to wherever the overlay's data
happens to point, which can crash or do anything else.

So the first set's tileset is the only number in the header the game uses
without checking for none.

## What the editor does

- The **Course** tab without the advanced mode offers none for the background
  and the foreground, and not for the tileset.
- The advanced mode still offers none for every tileset, as the header can hold
  it, and warns on the first set's when it is none.
- A level whose tileset is none draws as the game loads it: jyotyu, nohara, the
  background and the foreground, and nothing for the level's own tileset.
  Before, the editor tried to look up tileset 0xFFFF's files, failed, and drew
  nothing at all.
