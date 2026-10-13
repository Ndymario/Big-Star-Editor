# Lua backgrounds

A level has two scrolling layers behind its tiles. The level format calls block
2 the *background* and block 4 the *foreground*; on the hardware they are BG1
and BG3, and the foreground is the nearer of the two. The game holds 76 of each,
and what it does with one beyond drawing it -- drift its clouds, turn over its
lava, cycle a few colours -- is decided by tables and functions in overlay 0
indexed by its number.

A background script says what one of those numbers is called and what the game
does with it. Big Star loads background directories in this order, with the
first script for a layer and id winning:

1. `<project>/bigstar/backgrounds`
2. `bigstar/backgrounds` in each enabled module, in module order
3. the shipped `data/backgrounds` directory

## The shape of a script

A background script is data, as a [tile script](lua-tiles.md) is: it runs once
when it is loaded and has no `init`, no `render` and no renderer. Being Lua, it
may still build its tables with a loop.

```lua
return {
    layer = "background",
    id = 0,
    name = "Grassland (orange hills and clouds)",
    strips = {
        { throughRow = 344, drift = 0.125 },
        { scroll = 1.0 },
    },
}
```

| Field | Means |
| --- | --- |
| `layer` | `"background"` or `"foreground"`. Required. |
| `id` | the screen id, 0-255. Required. The game's tables stop at 75. |
| `name` | what the editor calls it. Required. |
| `strips` | how bands of its rows move sideways. |
| `tiles` | tiles the game overwrites as it runs. |
| `palette` | colours the game overwrites as it runs. |
| `swaps` | quarters of the image the game replaces once the layer has scrolled far enough. |

A script that sets only the first three names a background and nothing more,
which is all most of them need. The filename is free; the shipped ones are
`bg-NNN-name.lua` and `fg-NNN-name.lua` so they sort.

### `strips`

The game rewrites a layer's horizontal offset on every scanline from a table of
bands (`StageLayout::fillBgParallaxData`, fed to the hardware by an H-blank DMA).
`strips` is that table, top to bottom, in rows of the layer's 512-row image.
Each entry covers the rows up to and including `throughRow`; the last runs to
the bottom and needs none. Each has exactly one motion:

| Entry | Means |
| --- | --- |
| `scroll = n` | follows the layer's own scroll, times `n` |
| `drift = n` | moves on its own, `n` pixels a frame; negative runs the other way |
| `wave = { amplitude, speed, step }` | ripples: each row is offset by a sine of `amplitude` pixels, `step` further round than the row above and turning by `speed` a frame. A full turn is 65536. |

The game runs at sixty frames a second, so `drift = 0.125` is seven and a half
pixels a second.

`channel = n` on a drifting band names the counter it moves by. The game has
four, and every band on a channel adds its speed to it, so two bands that share
one both move at the sum of their speeds -- which is what the green hills of
background 12 do. A band that names no channel has a counter to itself.

### `tiles`

```lua
tiles = {
    file = "BG_ncg/d_2d_TEN_I_yakata_ncg.bin",
    interval = 5,
    frames = { 40, 44, 48, 52, 160, 164, 168, 172 },
    copies = {
        { layer = "foreground", to = 14, count = 4, from = 0 },
        { layer = "foreground", to = 46, count = 4, from = 16 },
    },
},
```

| Field | Means |
| --- | --- |
| `file` | the ROM path of the animation graphics, 8bpp tiles like any other |
| `frames` | the steps, in order and round again. A number is a tile of `file`; `{ tile, ticks = n }` is a step that lasts its own time. |
| `interval` | how many frames a step lasts when it does not say |
| `copies` | what every step copies: `count` tiles from the step's tile plus `from`, over tile `to` of a layer's graphics |

A copy lands in the script's own layer unless it names the other. That matters
more than it sounds: the two layers share one block of tiles in video memory,
and the game animates a castle by its *background's* number while writing into
its *foreground's* tiles. `to` counts from the first tile of the layer's own
graphics file.

### `palette`

```lua
palette = { file = "BG_ncl/d_2d_Bg1_TEN_W_yougan_ncl.bin", interval = 16, frames = 8, to = 80, count = 8, stride = 16 },
```

Every `interval` frames the game copies `count` colours over the layer's
palette from colour `to`, taking each frame's `stride` colours further into
`file` than the last. `stride` defaults to `count`.

### `swaps`

```lua
swaps = {
    { after = 256, quarter = "bottom-left", shows = "bottom-right" },
},
```

Some layers change themselves as the level goes by. A layer's image is four
256-pixel quarters, `top-left`, `top-right`, `bottom-left` and `bottom-right`,
and each entry says that once the layer has scrolled more than `after` pixels
sideways, `quarter` shows what `shows` holds. Scrolled back, it shows its own
again. Where two entries name one quarter the later wins.

`after` counts the layer's own scroll, not the level's: a foreground at half
speed passes 256 when the camera is 512 pixels into the view. The game does
this by rewriting the layer's map, and only for foreground 8, which is how
Peach's castle stands at the start of 1-1 and the hills then repeat without
it. The DS screen shows it; the still picture is the layer before any swap.

## Overriding

A later layer never has to restate the rest. To rename one background:

```lua
return { layer = "background", id = 12, name = "Mushroom plains" }
```

A definition replaces the whole of the one it shadows, so that script also
stops background 12's clouds: restate `strips` to keep them. A hack that adds a
77th background gives it a script with `id = 76`.

## What a level shows

Worked out from `StageLayout::loadCharacterGraphics`, `loadPalettes`,
`loadTilemaps` and `setupStuffForPlayer`:

| What | Comes from |
| --- | --- |
| graphics | the level header's background or foreground id |
| palette | the *palette id* of the layer's first block |
| screen | the *screen id* of the layer's block |
| scroll speeds, height, whether it shows | the block the **view** names |

The last row is why a background depends on the entrance: an entrance puts the
player in a view, the view names a block of each layer, and the block carries
the layer's scroll speeds (`scroll_x`, `scroll_y`), its vertical mode and
offset. A view whose block names no screen has no such layer, which is how the
ghost houses drop the foreground once the player is outside.

A block's `scroll_y` holds a speed in its low nibble and a mode in the next:

| Mode | The image |
| --- | --- |
| 0, 2 | hangs from the view's floor, its last 32 rows below it |
| 1, 3 | hangs from the view's ceiling, its first 32 rows above it |
| 4 | as 0, with its top half repeated above it: a sky that goes on up |
| 5 | as 1, with its bottom half repeated below it |

`vertical_offset` moves the image up eight pixels a step. The canvas draws each
view's layers where these put them with the camera resting at the anchored
edge, the image's first column on the view's left edge. That is a still
picture; where a layer really is depends on where the camera is.

### Where the camera puts a layer

With the screen's top left corner at `camera` in a view, a layer's scroll is
(`computeBottomBgScrolling`, `computeTopBgScrolling` and the six mode
functions; products are fixed point, rounded):

```
scroll x = (camera x - view left) * speed(scroll_x)
           kept between 0 and view width * speed(scroll_x)
scroll y = 320 - (view bottom - 192 - camera y) * speed(scroll_y) + base   floor-anchored
         = (camera y - view top) * speed(scroll_y) + base                   ceiling-anchored
base     = vertical_offset * 8, - 32 floor-anchored, + 32 ceiling-anchored
```

`speed` is the low nibble looked up in `bgScrollSpeeds`: 1, 1/2, 1/4, 1/8,
1.2, 3/4, 1/16, 3/32, 13/64, then 1 four times, 2, 4 and 8. The hardware
shows image row `scroll y` at the top of the screen and takes the whole pixels
of `scroll x`; a strip then replaces `scroll x` by `scroll x * factor`
(`scroll`) or adds its own movement to it (`drift`, `wave`).

A level that wraps around (header setting 0x20) does not keep `scroll x` in
range, and nor does the foreground of tileset 30. Three more things happen on
the way:

- Mode 4 never scrolls past row 320, and above row 16 shows only the top half
  of the image, over and over.
- Mode 5, once the screen's last row passes row 496, shows only the bottom
  half. The foreground copies it over the top half. The background instead
  switches to a 256-pixel-wide map of the bottom left quarter above the bottom
  right one, which is the same thing only for an image that repeats every 256
  pixels.
- A script's `swaps` take hold: foreground 8, once it has scrolled past 256
  pixels, shows its bottom right quarter in place of its bottom left.

### Where an entrance puts the camera

The player comes in at the entrance's position plus an offset its type names
(`Entrance::entranceInfoTable`): a normal entrance 8 pixels right and 16 down,
a door 16 and 16, a pipe facing up or left 16 and 32, one facing right 0 and
32, one facing down 16 and 16, a mini pipe 8 and 16. The 8 pixels of a normal
entrance are dropped when it is aligned (flag 0x40).

Sideways the camera centres the player, then stays inside the entrance's
view. Upright it moves only as far as brings the player between two lines down
the screen: 64 pixels plus the *top offset* of the view's camera settings, and
128 plus their *bottom offset*. A level that has just loaded starts its camera
at the top of the map, so the player ends up on the lower line; the view's
floor and then its ceiling have the last word.

A view's rows in the properties panel end with the camera settings it names,
so both offsets can be changed there. Views naming the same settings share
them; a view naming settings that do not exist reads the level's first, as the
game does, and gets its own the first time one is changed.

The **DS** menu in the canvas toolbar shows all of this, and chooses what is
drawn:

| Choice | Draws |
| --- | --- |
| No DS screen | nothing |
| DS screen under the cursor | a hollow 256 by 192 rectangle that follows the pointer |
| DS screen of selected entrances | a dashed one where each selected entrance opens the camera |
| Both | both |

Inside a rectangle both layers are drawn as the game scrolls them with its
camera there. The pointer's goes by the view the pointer is over; with both
shown and one entrance selected it goes by that entrance's view instead, so
the parallax can be tried from where the entrance opens. Dragging an entrance
moves its prediction with it. Each rectangle is labelled with its corner and
each layer's scroll, and an entrance's View ID row in the properties panel
says the same in words.

The prediction leaves out the zoom an entrance can set, and assumes the
camera was not already framing the player: arriving through a warp inside one
area, the camera stays where it was if the player is already between the two
lines, and puts them on the upper line if it comes from below.

In the canvas toolbar's background menu, **Animate backgrounds** plays the
strips, tiles and palettes; with it off the layers are drawn as the files hold
them. The Course tab of the properties panel names each layer's graphics,
screen and palette from these scripts, and a view's Background and Foreground
rows say which screen their block shows.

## What ships

`data/backgrounds` holds 152 scripts, one for each background and foreground
the game's tables have room for, named as NSMB Editor names them. A slot with
no files is `Empty`. The animations are transcribed from overlay 0:

| Script | The game's function | What it does |
| --- | --- | --- |
| `bg-001`, `bg-058` | `fgAnmYakata` | 8 steps of 5 frames, in the foreground's tiles |
| `bg-016` | `fgAnmKazangake` | 4 steps of 15 frames |
| `bg-032`, `bg-039` | `fgAnmYakata2` | 8 steps of 5 frames, in the foreground's tiles |
| `bg-047` | `fgAnmTorideBoss` | 8 steps of 5 frames |
| `bg-049` | `fgAnmYakataBoss` | 8 steps of 5 frames, in the foreground's tiles |
| `bg-055` | `fgAnmKoopaBoss` | 8 steps of 5 frames |
| `fg-008` | `bgAnmNoharaW1` | 4 steps of 7 frames |
| `fg-015`, `fg-016`, `fg-056` | `bgAnmKazan` | 40 steps over 448 frames |
| `fg-041`, `fg-045` | `bgAnmYakata` | 8 steps of 5 frames |
| `fg-063` | `bgAnmSabakuBoss` | 8 steps of 5 frames |
| `bg-007`, `bg-031`, `fg-007`, `fg-031` | the palette cycle | 8 colours, 8 steps |

The game's own names run the other way round -- its `fg` functions are indexed
by the background block and its `bg` ones by the foreground block -- so the
table follows the level format, as the rest of the editor does.

Strips are carried by 47 scripts, read from the two scanline tables. The
underwater ripple's numbers are the ones the game's liquid sets when a level
has water.

Two things the game does are not in the scripts. The volcano throws debris on
the last step of its eruption, which is an actor and not part of the layer. And
a block whose `scroll_x` is 0x100 or more also scrolls on its own; no shipped
level sets it. A strip's `scroll` factor only shows inside the DS screen, where
the camera has somewhere to be.

## Sandbox and limits

Background scripts get the same sandbox as actors and tiles -- Lua's `string`,
`math`, `table` and the pure parts of the base library, with no filesystem,
process, package or debug library, and no `load`, `dofile`, `loadfile` or
`require`. Each source file is limited to 1 MiB and each Lua state to 8 MiB.

A script that will not load is named in the log and costs nothing else. An
animation whose file is missing is logged and dropped, and the layer is drawn
without it. An animation whose parts never line up again is cut at a minute or
64 pictures, whichever comes first.
