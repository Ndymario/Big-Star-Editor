# M11 manual checklist

Automated coverage validates the schema, layer precedence, bit operations, generated fields, and grab-bound hit testing. Use this checklist with a user-provided NSMB DS project to verify the visual workflow.

## Definition layers

Big Star checks these actor directories in order and keeps the first definition for each `objectId` or `hash`:

1. `<project>/actors`.
2. `<enabled module>/actors`, in module order.
3. The app configuration directory's `actors` directory.
4. `data/actors` shipped with this source tree.

The plugin directory is only an input seam; M11 does not add a plugin runtime.

## Minimal schema example

```lua
return {
    schema = 1,
    objectId = 83,
    name = "Example actor",
    category = "Enemies / Ground",
    color = "#b5651d",
    shape = "ellipse",
    grabBounds = {x=-8, y=-8, width=16, height=16},
    renderBounds = {x=-8, y=-8, width=16, height=16},
    fields = {{key="mode", label="Mode", bitOffset=0, bitWidth=2,
               choices={{value=0,label="Off"},{value=1,label="On"}}}},
    graphics = readGraphics("I_kuribo256_ncg.bin"),
    palette = readPalette("I_enemy_ncl.bin"),
}
```

Bit offsets are zero-based. Field widths are 1–16 bits, must stay inside the 32-bit settings word, and may not overlap.

## Visual verification

- Open a ROM-backed level and confirm the Vanilla actors palette shows bold category headers, decoded thumbnails where available, and live search across both actor names and category names.
- Place or select a sprite whose class has a project definition. Confirm its named fields replace the two raw settings rows and edits preserve unrelated settings bits.
- Give that definition conspicuously different grab and render bounds. Confirm the canvas shape/image uses render bounds while clicks, marquee selection, scene sizing, and the selection outline use grab bounds.
- Add the same `objectId` to a lower-priority layer with a different name. Confirm the higher-priority name remains visible.
- Put a hash-based actor file in an enabled module's `actors` directory. Confirm its named fields, bounds, decoded image, and custom-palette icon load from that same file.
- Replace one resource with a missing filename, overlap two bit ranges, use an invalid colour, or change the schema. Reopen the project and confirm Big Star reports the actor-layer error instead of partially applying it.
