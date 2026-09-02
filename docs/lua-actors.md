# Lua actors

Every actor is one executable `.lua` file. It declares its metadata and settings
fields as data, and implements its drawing as functions. Big Star loads actor
directories in this order, with the first definition for an ID winning:

1. `<project>/bigstar/actors`, then `<project>/actors`
2. `bigstar/actors` then `actors` in each enabled module, in module order
3. the shipped `data/actors` directory

`bigstar/` is a module's whole contribution to the editor, one directory per
kind -- `actors` here, `tiles` in [Lua tiles](lua-tiles.md). The bare `actors/`
beside it is the older spelling and is still read, after it. There is no layer
in the user's config directory: an edit that lives there never reaches anybody
else on the project.

An actor names itself in exactly one of three ways:

| Key | For |
| --- | --- |
| `objectId` | a vanilla actor, by its runtime class ID |
| `stageObject` | a module actor, by the name its build writes to `build/generated/stageobjects.json` -- `"coop.CoopFlagActor.Default"` |
| `hash` | the same module actor, by the hash that name resolves to |

`stageObject` is the one to write. It resolves to exactly what `hash` would
have, and it survives the object being renamed, which the hash does not. It
needs a built stage-object catalog; without one the actor reports that and the
rest of the catalog loads.

## The shape of an actor

```lua
local images = {}

local function init(ctx)
    for i = 1, 10 do images["Arrow" .. i] = ctx.loadImage("images/Arrow" .. i .. ".png") end
    images.PipeJoint = ctx.loadImage("images/PipeJoint.png")
end

local function grabBounds(actor)
    local size = actor.fields.smaller and 16 or 32
    return {
        x = actor.pixelX - (size - 16) // 2,
        y = actor.pixelY - (size - 16),
        width = size,
        height = size,
    }
end

local function render(actor, renderer)
    local model = actor.fields.model_type
    local name = model <= 9 and ("Arrow" .. model + 1) or "PipeJoint"
    return renderer:draw(images[name], grabBounds(actor))
end

return {
    objectId = 77,
    name = "Arrow signboard",
    category = "Decoration",
    fields = {
        { key = "model_type", label = "Model type", bitOffset = 24, bitWidth = 4 },
        { key = "smaller", label = "Smaller", bitOffset = 16, bitWidth = 4, checkboxValue = 1 },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
```

- `init(ctx)` runs once when the catalog loads, and is the only place resources
  can be loaded. Keep the handles in an upvalue.
- `render(actor, renderer)` runs on every paint. Return truthy when the actor
  drew itself.
- `grabBounds(actor)` and `renderBounds(actor)` return the actor's box. If
  `renderBounds` is absent, `grabBounds` is reused. The editor applies them to
  hit testing, selection, scene fitting, the main canvas and the minimap.
  An actor that declares neither grabs as the 16x16 tile it sits on, which is
  the default `NSMBStageObj.GetGrabBounds()` starts from and what the upstream
  cases commented out as "don't need to be modified" keep.

## Coordinates

There is one coordinate system: **scene pixels**. `actor.pixelX` and
`actor.pixelY` are the actor's position (`x * 16`, `y * 16`) including any live
drag offset, and every box and destination rectangle is in the same space. That
is why a bounds table can be passed straight to a draw call:

```lua
return renderer:draw(image, grabBounds(actor))
```

`actor.x` and `actor.y` remain available as level grid coordinates.

## Reading settings

Never shift settings bits by hand. Every field the actor declares becomes a named
value on `actor.fields`, decoded by the same code that drives the properties
panel, so the canvas and the panel always agree:

```lua
local model = actor.fields.model_type      -- integer, with valueOffset and sign applied
if actor.fields.pipe_mode then ... end     -- checkbox fields are booleans
local raw = actor:field("odd-key-name")    -- for keys that are not identifiers
```

A checkbox field yields a **boolean**, because `0` is truthy in Lua. Reading a
key the actor does not declare raises an error rather than returning `nil`, so a
typo surfaces instead of quietly drawing the wrong thing.

`actor.settings` (the raw 32-bit value) and `actor.data[0]` through
`actor.data[5]` (NSMBe-compatible bytes: `Event 1`, `Event 2`, then settings from
most to least significant byte) are still there for anything a field cannot
express.

Field offsets address the complete 48-bit NSMBe data value. Bits 0-31 are
`settings`, bits 32-39 are Event 2, and bits 40-47 are Event 1. `valueOffset`,
`signed`, and `checkboxValue` reproduce NSMBe's numeric, signed, and checkbox
behavior. A field may be up to 31 bits wide.

A declared field never becomes the only way to edit a sprite. The panel's
**Settings** section keeps the raw NSMBe data value -- `Event 1`, `Event 2`, then
the settings word from its high byte down -- in a hex box beneath the fields, and
the two follow each other. Only the field the user turns is written back, so
changing the object ID re-reads the same value through a different actor instead
of rewriting it through the last one.

## Notes

A field's `notes` are read where the field is edited: the properties panel slides
them open under the field itself. The actor's own `notes` open the same way under
the sprite's **Object ID**, so selecting the object ID that names the actor is
what brings up what the actor documents about itself. `choices` stay on the
field's tooltip, being a value table rather than prose.

## Resources

`ctx` exposes four loaders, and each returns an opaque handle. A handle whose
file is missing is still valid; drawing it simply reports that nothing was drawn.

| Loader | Use |
| --- | --- |
| `ctx.loadImage("images/Goomba.png")` | a still shipped beside the actor script |
| `ctx.loadPalette(file, opts)` | a ROM palette, only used to feed `loadBitmap` |
| `ctx.loadBitmap(file, opts)` | indexed ROM graphics, bound to a palette |
| `ctx.loadTexture(file, opts)` | a TEX0 texture from an NSBMD or NSBTX |

Most NSMB actors are 3D models that the editor never renders as geometry, so
they draw from stills under `data/actors/images`. `loadImage` paths are relative
to the actor's own layer directory and may not climb out of it. Actors whose
graphics really are 2D tiles load from the ROM instead:

```lua
local function init(ctx)
    local palette = ctx.loadPalette("obj/I_enemy_ncl.bin", { colors = 16, bank = 2, transparent = true })
    sheet = ctx.loadBitmap("obj/I_enemy_ncg.bin", {
        bpp = 4,
        palette = palette,   -- required; bound once, not per draw
        width = 256,
        height = 64,         -- optional; inferred from the remaining tiles
        tileOffset = 0,      -- optional
    })
end
```

ROM resources are named by their full path from the filesystem root --
`enemy/sanbo_body.nsbmd`, not `sanbo_body.nsbmd` -- because two directories hold
files of the same name and a bare name cannot say which is meant. `loadImage` is
the exception: a still ships beside the script, so its path stays relative to
the script's own layer directory.

`loadBitmap` accepts raw tiled 4bpp/8bpp data and NCGR containers; raw data needs
a pixel width divisible by 8, and NCGR dimensions are used when present.
`loadPalette` accepts raw BGR555 data and NCLR containers, and multiplies `bank`
by `colors`, so specify the bank size when selecting a bank. `loadTexture`
options `texture` and `palette` select zero-based entries.

Decoded assets are cached by filename and decode options across every actor, so
two actors sharing a sheet decode it once. Loading is unavailable once `init`
returns: calling a loader from `render` raises, which keeps painting free of
file I/O.

## Drawing

Every renderer method takes a drawable handle and returns whether it drew:

- `renderer:draw(drawable, opts)`
- `renderer:drawTile(drawable, tileIndex, opts)` — an 8x8 source tile
- `renderer:drawFrame(drawable, frameIndex, frameWidth, frameHeight, opts)` —
  frames arranged left-to-right, then top-to-bottom
- `renderer:drawText(text, opts)`

`opts` accepts `x`, `y`, `width`, `height`, `sourceX`, `sourceY`, `sourceWidth`,
`sourceHeight`, `flipX`, `flipY`, `rotation` in degrees, and `opacity` from 0
through 1. Omitted positions default to the actor's pixel position, and omitted
sizes to the selected source size. Rotation and flipping are applied about the
destination rectangle's centre.

A drawable also reports its own `width` and `height` in pixels, which is what a
script needs to place a rotated image or to step across a sheet.

Shapes take a rectangle rather than a drawable, and a `color` written as
`"#rgb"`, `"#rrggbb"` or `"#rrggbbaa"` (or a packed `0xRRGGBBAA` integer). They
exist because a handful of actors outline their reach rather than draw a still:

- `renderer:fillRect(opts)`
- `renderer:strokeRect(opts)` — `strokeWidth` sets the thickness
- `renderer:fillEllipse(opts)`
- `renderer:drawLine(opts)` — `x`, `y` to `x2`, `y2`

## The `gfx` helpers

Two patterns come up in almost every actor: an image whose transform is decided
somewhere other than the draw call, and a destination expressed as the top-left
of the *drawn* result rather than as a rectangle to rotate in place. The loader
runs a small Lua layer over the renderer so neither has to be restated in three
hundred files:

| Helper | Does |
| --- | --- |
| `gfx.transform(image, rotation, flipX, flipY)` | pairs an image with a transform, to apply wherever it is drawn |
| `gfx.blit(renderer, image, x, y, w, h, source)` | draws so the visible result's top-left lands on `x, y` |
| `gfx.width(image)` / `gfx.height(image)` | pixel size of an image or a transformed image |
| `gfx.colored(rect, color)` | adds a colour to a rectangle, for the shape methods |

`gfx` is a convenience, not a requirement. An actor that knows its own geometry
should call `renderer:draw` directly, which is what the hand-written ports do.

Multiple calls compose one actor, and that is the normal case:

```lua
local function render(actor, renderer)
    local y = actor.pixelY
    local drawn = false
    for _ = 2, actor.fields.height do
        drawn = renderer:draw(gfx.body, { x = actor.pixelX, y = y, width = 18, height = 16 }) or drawn
        y = y - 16
    end
    return renderer:draw(gfx.head, { x = actor.pixelX, y = y - 9, width = 18, height = 25 }) or drawn
end
```

## When an actor does not draw

If `render` returns false, throws, or the actor has no `render` at all, the
canvas draws the stock marker: a plain box with the actor's number. There is no
generic shape or colour representation; an actor either draws itself or is a
numbered box, which keeps an unported actor visibly unported.

Returning false is not the same as drawing nothing. Whatever the script drew
still paints, and the marker goes on top — that is how an actor draws a range
indicator or a guide line while leaving its body as the numbered box.

`init` failing costs that actor its renderer and nothing else. Its metadata,
fields and bounds still load, and the rest of the catalog is unaffected, so a
module can override an actor without making the catalog depend on every optional
asset being installed.

## Sandbox and limits

Actor scripts get Lua's `string`, `math`, `table` and the pure parts of the base
library — enough to format numbers into labels and resource names. The
filesystem, process, package and debug libraries are absent, as are `load`,
`dofile`, `loadfile` and `require`. Big Star limits each source file to 1 MiB,
each Lua state to 8 MiB, each callback to 200,000 VM instructions, and each
render to 4096 draw calls. Draws past that limit are dropped rather than raised,
so an actor given nonsense settings still shows the part that fits.

## The shipped catalog

`data/actors` holds all 305 vanilla class definitions. Metadata, fields and notes
are imported from NSMB Editor 5's `stageobjsettings.xml`; bounds and drawing are
ported from `NSMBStageObj.GetGrabBounds()` and `NSMBStageObj.Render()`.

Every upstream case that can be expressed against this API is ported: 204 actors
have a `render`. The 89 actors that draw nothing upstream keep a bounds function
and the numbered marker, which is exactly what the old editor shows for them.

Ten are not ported, because their drawing needs context an actor script does not
get:

| Actors | Needs |
| --- | --- |
| 206, 243, 245, 247 | tiles out of the level's own tileset |
| 274 | the positions of other actors in the level |
| 277, 278, 279, 281 | gradients and level-wide liquid fills |
| 228, 230 | arc strokes and arbitrary-angle image rotation |

`data/actors/images` holds NSMBe's sprite stills, copied under GPL-3.0; see
`docs/licenses.md` and the README in that directory.

Most of those `render` functions were transpiled from the upstream C# rather than
written, and read like it. `docs/actor-cleanup-brief.md` is the brief for
rewriting them by hand -- above all, for replacing the raw `actor.data` bit
arithmetic they still contain with declared, named fields.
