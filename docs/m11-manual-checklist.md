# M11 manual checklist

Automated coverage validates the schema, layer precedence, bit operations, generated fields, and grab-bound hit testing. Use this checklist with a user-provided NSMB DS project to verify the visual workflow.

## Definition layers

Big Star checks these layers in order and keeps the first definition for each `objectId`:

1. `<project>/plugins/*/sprite-definitions.json` in lexical plugin-directory order.
2. `<project>/sprite-definitions.json`.
3. The app configuration directory's `sprite-definitions.json`.
4. `data/sprite-definitions.json` shipped with this source tree.

The plugin directory is only an input seam; M11 does not add a plugin runtime.

## Minimal schema example

```json
{
  "schema": "bigstar.sprite-definitions/1",
  "sprites": [{
    "objectId": 83,
    "name": "Example actor",
    "category": "Enemies / Ground",
    "color": "#b5651d",
    "shape": "ellipse",
    "grabBounds": {"x": -8, "y": -8, "width": 16, "height": 16},
    "renderBounds": {"x": -8, "y": -8, "width": 16, "height": 16},
    "fields": [{
      "key": "mode",
      "label": "Mode",
      "bitOffset": 0,
      "bitWidth": 2,
      "choices": [{"value": 0, "label": "Off"}, {"value": 1, "label": "On"}]
    }]
  }]
}
```

Bit offsets are zero-based. Field widths are 1–16 bits, must stay inside the 32-bit settings word, and may not overlap.

## Visual verification

- Open a ROM-backed level and confirm the Vanilla actors palette shows bold category headers, decoded thumbnails where available, and live search across both actor names and category names.
- Place or select a sprite whose class has a project definition. Confirm its named fields replace the two raw settings rows and edits preserve unrelated settings bits.
- Give that definition conspicuously different grab and render bounds. Confirm the canvas shape/image uses render bounds while clicks, marquee selection, scene sizing, and the selection outline use grab bounds.
- Add the same `objectId` to a lower-priority layer with a different name. Confirm the higher-priority name remains visible.
- Place a module object. Confirm it has no generated named fields and draws as the generic purple box.
- Introduce an unknown property, overlapping bit range, invalid colour, or unsupported schema. Reopen the project and confirm Big Star reports the layer error instead of partially applying it.
