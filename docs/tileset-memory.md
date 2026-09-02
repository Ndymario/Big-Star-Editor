# Tileset memory

A level does not load one tileset. It loads three, into one address space, and
they refer to each other across it. This is what the editor has to reproduce to
draw a Map16 block the way the game draws it.

Everything below was read out of the retail US ROM (`A2DE`) in Ghidra rather
than inferred from the data, and the addresses name that build. The three
loaders are:

- `loadJyotyuAndSubNohara @ 020adff0` -- jyotyu and nohara, once per level load.
- `StageLayout::loadCharacterGraphics @ 020bcfec` -- the level's own tileset.
- `StageLayout::onCreate @ 020bb9a8` -- the level tileset's Map16.
- `Stage::loadJyotyuPalette @ 020a16a8` -- jyotyu's palette.

## The three address spaces

| | jyotyu | level tileset | nohara (subunit) |
| --- | --- | --- | --- |
| BG2 character memory | `0x0000` -- tile **0** | `+0x3000` -- tile **192** | `+0xa000` -- tile **640** |
| BG2 extended palette | `0x4000` -- palette **0-1** | `0x4400`, `0x800` -- palettes **2-5** | `0x4c00`, `0x400` -- palettes **6-7** |
| Map16 blocks | `+0`, cap `0x800` -- blocks **0-255** | `+0x800`, cap `0x1800` -- blocks **256-1023** | `+0x2000`, cap `0x1000` -- blocks **1024-1535** |

Character memory divides by `0x40` to give a tile number, extended palette by
`0x200` to give a palette number, and the Map16 buffer by `8` to give a block
number. `ptrToMap16` is a `u16*`, so the `+0x400` and `+0x1000` in the
decompiled source are `0x800` and `0x2000` bytes.

The partition is exact, not conventional. The character gap left for the level
tileset is `0xa000 - 0x3000 = 0x7000`, which is 448 tiles, and the largest
tileset in the retail ROM is 448 tiles to the tile. The three Map16 caps sum to
`0x800 + 0x1800 + 0x1000 = 0x3000`, which is the whole of the single
`allocFromCacheTable(0x3000)` the buffer comes from. Nothing can overlap, and
nothing is spare.

These are the numbers `TilesetResourceFiles::tile_offset`,
`palette_offset` and `tile_page_offset` carry. A page offset is a Map16 block
base divided by 256: `0`, `1` and `4` are blocks `0`, `256` and `1024`.

## What that means for a Map16 entry

A Map16 quadrant is a DS BG screen entry: ten bits of tile, one bit each of
horizontal and vertical flip, four bits of palette. Those indices are absolute
-- they address the whole space above, not the file they were read from.

So an entry in the level tileset's Map16 naming tile 5 is naming a *jyotyu*
tile, because the level tileset's own graphics start at 192. The editor decodes
entries relative to the owning tileset's offsets, which makes such an entry
negative, and `ScreenMapEntry` keeps it negative rather than clamping it: that
is what lets the file be written back byte-identically.

Rendering a tileset alone therefore cannot draw those entries -- the graphics
they name are in a different file. `compose_tileset_memory` builds the
composed space instead, and `render_composed_map16_sheet` draws against it, so
they resolve to the tiles the game would show. NSMB Editor does not do this;
`Tilemap.reRender` bounds every tile against the one tileset's own `tileCount`,
so those blocks are blank there.

Nothing restricts this. A screen entry is absolute by construction, and
`handle3DTile` below keys off block numbers, never tile indices, so no stage of
the pipeline ever checks that a block stays inside its own tileset.

The retail data uses that, sparingly. Counting only Map16 blocks that some
object definition reaches, across all 76 valid tilesets:

| Owner | Blocks | Names | Drawn? |
| --- | --- | --- | --- |
| jyotyu | 17 | level tileset tiles 192-242 with nohara's palettes 6-7 | **no** -- all 17 are 3D-tile blocks, see below |
| level tileset | 37 | jyotyu tile 1, with jyotyu's palette 0 | yes |
| level tileset | 4 | its own tile 0, with jyotyu's palette 0 | yes |
| nohara | 18 | jyotyu tiles 70-73 and 102-108, with nohara's palettes 6-7 | yes |

The level tileset's 37 are `d_2d_I_M_tikei_yakata` (castle -- tilesets 1, 32, 41,
45) borrowing jyotyu tile 1 for 30 blocks and `d_2d_I_M_tikei_kinoko`
(mushroom -- 13, 14, 33, 50) for 7, and jyotyu tile 1 is a real gradient-shaded
tile, not filler. The 4 palette-only ones are `d_2d_S_M_tikei_obake_soto`
(tileset 40). Nohara's 18 are the same in every tileset, reached by nohara
object 5, and they recolour jyotyu's graphics with nohara's palette.

So composing is needed -- just not for jyotyu, whose every cross-tileset block
turns out to be one the game never draws.

## Two details the editor does not model yet

**Jyotyu's palette is chosen per level, by the background screen ID.**
`Stage::loadJyotyuPalette` indexes a four-entry `JyotyuPalettesTable` with the
result of `GetJyotyuPaletteForFG @ 020baa2c`, which is
`JYOTYU_NCL_FOR_BG[background scrID]` -- a `byte[84]` at `020c93b8`, with IDs
above 75 falling back to entry 0. The four entries are the normal, `_B_`, `_R_`
and `_W_` palettes. `resolve_level_graphics` always takes
`d_2d_A_J_jyotyu_ncl.bin`, which is entry 0. NSMB Editor does do this: its
`LoadTilesets(TilesetID, JyotyuPalOverride)` reads the same table with the
level's background NSC id, and only its tileset editor falls back to the
hardcoded index 8. The level's `background_screen_id` is already in
`LevelTilesetReferences`, so this is a lookup away.

**Palette 15 is occupied.** `loadJyotyuAndSubNohara` uploads
`d_2d_A_J_jyotyu_F_ncl.bin` to `0x5e00`, one 256-colour block. Nothing reads it
today, and an entry naming palette 15 renders clear.

## Blocks the game never draws

A level's tile array does not reach the tilemap unchanged.
`StageLayout::renderTileRow @ 020afd4c` passes every block through
`StageLayout::handle3DTile @ 020aff70` and copies whatever *that* returns:

```c
idx = handle3DTile(this, pLevelChunks[chunk][...], curX, y);
MIi_CpuCopy16(ptrToMap16 + idx * 4, getTilesetTilemap(this, curX, y), 4);
```

Both tilemap writers go through it: `renderTileRow` above and
`renderTileColumn @ 020afe68`. There is no other path that reaches
`ptrToMap16`, so no block escapes the substitution.

`handle3DTile` looks the block up in a set of tables. For blocks `0x70`-`0xff`
the table is a `byte[144]` at `020c7c7c`, indexed by the block number itself,
and **113 of its 144 entries are non-zero**. A non-zero entry means the block
is a 3D tile: the function records the entry in `pLevel3dTiles` and returns
block **0** -- jyotyu's empty block -- so the block's own Map16 is never drawn
on the BG at all.

This is the answer to "why is there no black square in game": nothing clears
anything, because the block is never written. `handle3DTile` returns 0, so the
`MIi_CpuCopy16` above copies `ptrToMap16[0]`, and jyotyu's block 0 is four
entries of `tile 0, palette 0` whose tile is 64 pixels of colour index 0 --
fully transparent. The BG shows through to the background layer, and the 3D
quad is drawn in front of it.

`StageLayout::Draw3DTiles @ 020af0cc` then turns each recorded id into
`source3dTiles[id - 1]`, a `TextureInfo` that `3DTilesSetup @ 020f4660` built
from `3DTilesetTiles_list @ 02121ec0` (142 entries of `{FID fileId, u32 texNo,
u16 plttOffset}`), and `Draw3DTile @ 020f438c` draws it as a textured quad on
the 3D engine. The pipes, fences and pillars a player sees are polygons, not
tiles.

Since all of `0x70`-`0xff` falls inside jyotyu's 0-255, this is entirely a
jyotyu concern. Jyotyu Map16 block 171 (`0xab`), for instance, has table entry
17, so it draws `3DTilesetTiles_list[16]` -- texture 24 of a pipe NSBTX -- and
its Map16 bytes, `tile 196-229 with palette 7`, are never read by the hardware.

That answers the question the offsets above left open. Those blocks pair
*level tileset* tiles (192-242) with *nohara's* palettes (6-7) because nothing
constrains them: they are placeholder bytes behind a polygon. Composing the
three tilesets renders them as whatever those bytes happen to name -- for
block 171, a black square -- which is exactly as wrong as leaving them blank,
just differently wrong.

The same function handles four more cases, only some of which blank the block:

| Blocks | Condition | Drawn as a tile? |
| --- | --- | --- |
| `0x300`-`0x30c` fences, `0x315`-`0x31d` unused pillar | boss-ish tilesets (1, 0x27, 0x29, 0x2d, 0x2f, 0x31, 0x3a) | no, 3D only |
| `0x130`-`0x131` desert quicksand, `0x1d9`-`0x1da` Bowser Jr quicksand | tilesets 2, 6, 0x14, 0x34, 0x35 / 0x2f | no, 3D only |
| `0x5b0`-`0x5bb`, `0x5f0`-`0x5f8` end-of-level tower | always | **yes**, and a 3D tile as well |
| `_dWaterObjectTileIDs` water surfaces | always | **yes**, and a 3D tile as well |
| `0x311`-`0x314` thin fence | boss-ish tilesets | yes, but remapped to block - 4 |
| `0x320`-`0x323`, `0x32c`-`0x330` | boss-ish tilesets | no, and no 3D tile either |

Big Star reads that table out of Lua rather than out of the editor: `data/tiles`
carries the retail mapping, and a module can extend or override it. See
[Lua tiles](lua-tiles.md).

## What NSMB Editor does about it

It hardcodes the answer. `NSMBGraphics.LoadTilesets` fills
`Tilesets[0].Overrides`, a `block -> sprite index` table written out by hand,
and `NSMBTile.render` draws the Map16 block and then blits a 16x16 sprite from
`Resources/tileoverrides.png` (2512x16 -- 157 hand-drawn sprites) over the top.

That table is a transcription of the ROM's. Its static entries cover all 113
3D blocks, and its `block -> sprite` mapping is a bijection with the game's
`block -> 3D tile id` mapping across all 71 distinct ids. The extra entries
(blocks 1-8, 36, 147, 148, plus the question/brick/invisible blocks that
`RepatchBlocks` fills in) are animated tiles, a separate concern.

Note that NSMB Editor paints *over* the block rather than replacing it, so the
underlying Map16 is still drawn -- it just never shows, because the sprites are
opaque.

`header->tileset[1]`, which `onCreate` stores in `field14_0x476`, is not part of
this partition and has not been chased.
