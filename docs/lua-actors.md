# Lua actors

Every actor is one executable `.lua` file containing its metadata, settings fields, bounds, resource declarations, and drawing logic. Big Star loads actor directories in this order, with the first definition for an ID winning:

1. `<project>/actors`
2. `actors` in each enabled module, in module order
3. `<Big Star config>/actors`
4. the shipped `data/actors` directory

Vanilla actors use their runtime class ID as `objectId`. Module actors use the stable hash written to `build/generated/stageobjects.json` as `hash`. Exactly one is required, so enabled modules use the same API as the vanilla catalog.

## Rendering model

An actor file is evaluated once when the catalog loads. Load ROM resources into locals at the top level, then return an `onRender(actor, renderer)` callback. The Lua state and those handles stay alive; decoded assets are cached by resolved filename and decode options rather than reread for every paint.

```lua
local texture = loadTexture("kuribo.nsbmd")

local function render(actor, renderer)
    renderer:draw(texture, {
        x = actor.pixelX,
        y = actor.pixelY - 2,
        width = 16,
        height = 18,
    })
end

return {
    schema = 1,
    objectId = 83,
    name = "Goomba",
    category = "Enemies / Ground",
    onRender = render,
}
```

`loadTexture(filename, options)` reads a TEX0 texture and palette from an NSBMD or NSBTX. `options.texture` and `options.palette` select zero-based entries and default to zero. Big Star renders the texture; it does not attempt to reproduce the model's 3D geometry or animation.

Indexed Nintendo DS graphics use separate bitmap and palette handles:

```lua
local image = loadBitmap("I_enemy_ncg.bin", {
    bpp = 4,
    width = 256,
    height = 64,       -- optional; inferred from the remaining tiles
    tileOffset = 0,    -- optional
})
local palette = loadPalette("I_enemy_ncl.bin", {
    colors = 16,
    bank = 2,
    transparent = true,
})

local function render(actor, renderer)
    renderer:drawTile(image, palette, actor.data[5] & 3, {
        x = actor.pixelX,
        y = actor.pixelY,
        width = 16,
        height = 16,
        flipX = (actor.data[4] & 1) ~= 0,
    })
end
```

`loadBitmap` accepts raw tiled 4bpp/8bpp data and NCGR containers. Raw data requires a pixel width divisible by 8; NCGR dimensions are used when present. `loadPalette` accepts raw BGR555 data and NCLR containers. Palette `bank` is multiplied by `colors`, so specify the bank size when selecting a bank.

The renderer methods are:

- `renderer:draw(texture, options)`
- `renderer:draw(bitmap, palette, options)`
- `renderer:drawTile(bitmap, palette, tileIndex, options)` for an 8x8 source tile
- `renderer:drawFrame(bitmap, palette, frameIndex, frameWidth, frameHeight, options)` for frames arranged left-to-right, then top-to-bottom

All methods return whether an image was available and drawn. Options are `x`, `y`, `width`, `height`, `sourceX`, `sourceY`, `sourceWidth`, `sourceHeight`, `flipX`, `flipY`, `rotation` in degrees, and `opacity` from 0 through 1. Omitted positions default to the actor's pixel position, and omitted sizes default to the selected source size. Multiple calls compose one actor.

For example, Pokey is ported directly from NSMBe's `NSMBStageObj.cs` rather than from its PNG previews:

```lua
local body = loadTexture("sanbo_body.nsbmd")
local head = loadTexture("sanbo_head.nsbmd")

local function render(actor, renderer)
    local y = actor.pixelY
    for _ = 0, actor.data[5] & 7 do
        renderer:draw(body, { x = actor.pixelX, y = y, width = 18, height = 16 })
        y = y - 16
    end
    renderer:draw(head, { x = actor.pixelX, y = y - 9, width = 18, height = 25 })
end
```

If `onRender` returns `false`, produces no draw calls, throws an error, or its ROM resources are absent, the canvas uses the legacy resource preview or the schematic shape. This lets one actor override another in a module without making the catalog depend on every optional asset being installed.

## Actor data and bounds

The callback receives:

- `actor.x`, `actor.y`: level grid coordinates
- `actor.pixelX`, `actor.pixelY`: scene coordinates (`x * 16`, `y * 16`), including a live drag offset
- `actor.objectId`, `actor.type`, optional `actor.hash`, and `actor.valid`
- `actor.settings`: the raw 32-bit settings value
- `actor.data[0]` through `actor.data[5]`: NSMBe-compatible bytes (`Event 1`, `Event 2`, then settings from most to least significant byte)

Bounds can also depend on actor data. Return bounds relative to the actor position from `getGrabBounds(actor)` and optionally `getRenderBounds(actor)`. If the latter is absent, the grab-bounds function is reused. The editor applies these functions to hit testing, selection, scene fitting, the main canvas, and the minimap.

```lua
local function bounds(actor)
    local height = 16 * (2 + (actor.data[5] & 7)) + 9
    return { x = 0, y = 16 - height, width = 18, height = height }
end

return {
    schema = 1,
    objectId = 98,
    name = "Pokey",
    grabBounds = { x = 0, y = -25, width = 18, height = 41 }, -- fallback
    renderBounds = { x = 0, y = -25, width = 18, height = 41 },
    getGrabBounds = bounds,
    getRenderBounds = bounds,
    onRender = render,
}
```

## Metadata and settings fields

Static metadata remains in the returned table:

```lua
return {
    schema = 1,
    objectId = 83,
    name = "Goomba",
    category = "Enemies / Ground",
    notes = "Walks until it meets an obstacle.",
    known = true,
    complete = true,
    color = "#b5651d",
    shape = "ellipse", -- box, ellipse, or diamond
    grabBounds = { x = -8, y = -8, width = 16, height = 16 },
    renderBounds = { x = -8, y = -8, width = 16, height = 16 },
    sourceFiles = { "/enemy/kuribo.nsbmd" },
    fields = {
        {
            key = "size",
            label = "Size",
            bitOffset = 0,
            bitWidth = 2,
            notes = "Actor-specific setting help.",
            valueOffset = 0,
            signed = false,
            choices = {
                { value = 0, label = "Small" },
                { value = 1, label = "Big" },
            },
        },
    },
}
```

Field offsets address the complete 48-bit NSMBe data value. Bits 0-31 are `settings`, bits 32-39 are Event 2, and bits 40-47 are Event 1. `valueOffset`, `signed`, and `checkboxValue` reproduce NSMBe's numeric, signed, and checkbox behavior. A field may be up to 31 bits wide.

`sourceFiles` is documentation. The older `graphics = readGraphics(...)` plus `palette = readPalette(...)`, or `model = readModel(...)`, remain supported as a fallback while actors are migrated. New rendering logic should use `loadBitmap`, `loadPalette`, and `loadTexture`; direct resource paths and string values are rejected so scripts cannot escape the active project/module resolver.

## Sandbox and limits

Actor scripts run without Lua's filesystem, process, package, debug, or standard libraries. Big Star limits each source file to 1 MiB, each Lua state to 8 MiB, each callback to 200,000 VM instructions, and each render to 256 draw calls. Resource loads are only available during initial script evaluation, keeping painting deterministic and free of file I/O.

## Refreshing the vanilla catalog

The shipped directory contains all 305 class definitions imported from NSMB Editor 5. Regenerate it with:

```sh
python3 tools/import_nsmbe_actors.py /path/to/NSMB-Editor --output data/actors
```

The importer carries across names, categories, notes, documentation status, fields and choices, source-file notes, and grab bounds derived from `NSMBStageObj.GetGrabBounds()`. It never copies NSMBe's bundled PNGs or Nintendo ROM data. Rendering must be ported case-by-case to the ROM-backed API; actor 98 is the first settings-dependent composite port.
