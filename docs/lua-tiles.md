# Lua tiles

A level's Map16 does not decide everything that appears on the tile grid. The
game hands a few hundred blocks to the 3D engine and draws nothing from their
Map16 at all: pipes, wire fences, pillars, quicksand, the end-of-level tower and
water surfaces are textured quads. Their Map16 bytes are dead placeholders, so
an editor that draws them faithfully shows black squares nobody ever sees in
game. `docs/tileset-memory.md` has the mechanism.

A tile script says which blocks those are and what the game draws in their
place. Big Star loads tile directories in this order, with the first script that
speaks for a tileset and claims a block winning:

1. `<project>/bigstar/tiles`
2. `bigstar/tiles` in each enabled module, in module order
3. the shipped `data/tiles` directory

## The shape of a script

A tile script is data. Unlike an actor it runs no code at paint time, so it has
no `init`, no `render` and no renderer:

```lua
return {
    name = "vanilla.jyotyu3d",
    blocks = {
        [171] = { "polygon_unit/I_dokan.nsbtx", "I_do_yoko_lu" },
        [172] = { "polygon_unit/I_dokan.nsbtx", "I_do_yoko_u", palette = 256 },
    },
}
```

| Field | Means |
| --- | --- |
| `name` | what the log calls this script. Defaults to the filename. |
| `blocks` | absolute Map16 block number to what the game draws there. Required. |
| `replaces` | `true` (the default) when the block's own Map16 never draws, `false` when it draws and the texture goes over it. |
| `tilesets` | the tileset ids this script speaks for. Omit it to mean every tileset; an empty list is an error rather than a silent "all". |

Block numbers are absolute across the three tilesets a level loads: jyotyu is
0-255, the level tileset 256-1023, nohara 1024-1535.

A `blocks` entry is a table:

| Entry | Means |
| --- | --- |
| `{ file, texture }` | a ROM path and a TEX0 texture, by name or by zero-based index |
| `palette = n` | start `n` colours into the palette. The green, red and yellow pipes are one set of texels read at 0, 256 and 512 into `I_dokan`'s single 768-colour palette. |
| `paletteIndex = n` | which palette in the container, when it holds more than one. Defaults to 0. |
| `{ remap = block }` | the game rewrites this block to another block rather than to a texture |
| `{}` | the block is claimed and draws nothing |

The file is a ROM path from the filesystem root -- `polygon_unit/I_dokan.nsbtx`,
not `I_dokan.nsbtx` -- and it is read through the same object the canvas reads
through, so a module that replaces an NSBTX gets its own texture without
touching the script. Naming the texture rather than its index is worth the extra
characters: an index says nothing about what it draws, and moves if a module
rebuilds the container.

## Overriding

A later layer never has to restate a file. To change one pipe, claim one block:

```lua
return {
    name = "mymod.wide-pipe",
    blocks = { [171] = { "z_new/mymod/pipes.nsbtx", "wide_lu" } },
}
```

The shipped `vanilla.jyotyu3d` still supplies the other 112.

To make a block draw its own Map16 again -- because your hack repoints the
game's 3D tile table -- claim it and draw nothing over it:

```lua
return { name = "mymod.no-pipes", replaces = false, blocks = { [171] = {} } }
```

## What ships

`data/tiles` holds the retail mapping, transcribed from the tables
`StageLayout::handle3DTile` reads, one script per case:

| Script | Blocks | `replaces` |
| --- | --- | --- |
| `vanilla-pipes.lua` | 113 of jyotyu's `0x70`-`0xff` | true |
| `vanilla-fences.lua` | `0x300`-`0x30c`, boss-ish tilesets | true |
| `vanilla-pillar.lua` | `0x315`-`0x31d`, boss-ish tilesets | true |
| `vanilla-thin-fence.lua` | `0x311`-`0x314` remapped, `0x320`-`0x323` and `0x32c`-`0x330` blanked | true |
| `vanilla-quicksand-desert.lua` | `0x130`-`0x131`, sand tilesets | true |
| `vanilla-quicksand-boss.lua` | `0x1d9`-`0x1da`, tileset `0x2f` | true |
| `vanilla-tower.lua` | `0x5b0`-`0x5bb`, `0x5f0`-`0x5f8` | **false** |
| `vanilla-water.lua` | `0x93`, `0x94`, `0x29` | **false** |

176 blocks, 163 of them with a texture. NSMB Editor covers the same ground with
`Resources/tileoverrides.png`, 157 sprites drawn by hand; these read the ROM's
own textures, so a hack that redraws a pipe sees the pipe it redrew.

The tower and water blocks keep their Map16 because the game keeps it: those two
cases record a 3D tile and return the block unchanged. Sixteen of the twenty-one
tower blocks come out pixel-identical either way, which is Nintendo matching the
tile to the quad so the seam does not show.

## Where the substitution happens

`load_level_tilesets` applies the catalog once, where a block number becomes an
image, so the level canvas, the object previews and the tileset editor's Map16
sheet all agree without any of them knowing about it. It is render-only: the
`EditableTileset` a save writes back is untouched, and Map16 round trips stay
byte-identical.

The tileset editor's Map16 tab has a **Show 3D tiles** toggle. Unticked, the
sheet shows the bytes the file holds, which is what you want when editing them.

## Sandbox and limits

Tile scripts get the same sandbox as actors -- Lua's `string`, `math`, `table`
and the pure parts of the base library, with no filesystem, process, package or
debug library, and no `load`, `dofile`, `loadfile` or `require`. Each source
file is limited to 1 MiB and each Lua state to 8 MiB.

A script that will not parse fails the load and names itself, because that is a
mistake in the file. A texture that will not resolve costs its own block and
nothing else, so a module missing one asset still contributes the rest.
