# Cleaning up the machine-ported actors

## What happened

`data/actors/*.lua` holds 305 actor programs. 204 of them have a `render`
function. Ten of those were written by hand and read well. **194 were produced by
a transpiler** that walked NSMB Editor 5's `NSMBStageObj.GetGrabBounds()` and
`NSMBStageObj.Render()` and emitted Lua statement by statement.

The transpiled files are *correct* — the whole catalog loads, and every actor's
bounds and render survive a spread of settings values in
`libs/nsmb-project/tests/lua_actors_tests.cpp` — but they read like decompiler
output, because that is what they are. This document is the brief for rewriting
them by hand, one file at a time, into the shape a person would have written.

Read `docs/lua-actors.md` first. It is the API reference; this document is only
the cleanup task.

## The job

For each file under `data/actors/` that carries the header comment
`-- Ported from NSMB Editor 5's NSMBStageObj.cs case N.`, rewrite `grabBounds`
and `render` so they say what the actor *is*, then verify nothing changed.

Do one actor per commit-sized unit of work. Do not batch-edit with a script —
the whole point is that a script produced what is there now.

## Rules

### 1. No raw settings bytes

This is the most important one. Every read of `actor.data[n]` must become a
declared, named field.

```lua
-- No.
local LiftLength = d[5] % 0x8
local dist = (d[4] % 0x10) * 16

-- Yes.
local length = actor.fields.platform_width
local distance = actor.fields.tiles_to_move * 16
```

`d[5] % 0x8 + 1`, `(d[2] & 0xF0) // 0x10`, `d[3] >> 5` and friends are all
unacceptable. The transpiler substituted a field name only where a declared field
matched the bit range *exactly*; everywhere else it left the arithmetic in place.
That leaves 213 raw reads across 53 files.

When no declared field covers the bits the drawing reads:

1. Work out the bit range. `actor.data` indexes map onto the 48-bit value as
   `data[0]` → bits 40-47, `data[1]` → 32-39, `data[2]` → 24-31, `data[3]` →
   16-23, `data[4]` → 8-15, `data[5]` → 0-7. So `d[5] % 0x8` is bits 0-2.
2. **Add the field to the `fields` table** with a `key`, a `label` a person would
   recognise, `bitOffset` and `bitWidth`. Use `checkboxValue` when the drawing
   only tests one value, so the field reads as a boolean.
3. Read it as `actor.fields.<key>`.

Adding the field is not a side effect — the properties panel wants it too, and it
is the only way the canvas and the panel are guaranteed to decode the same bits
the same way. If an existing field overlaps the range but is wider, prefer
splitting or narrowing it over leaving raw arithmetic behind; say so in the
commit message when you change an existing field's shape.

`actor.settings` and `actor.data` stay in the API for the rare thing a field
genuinely cannot express. Reaching for them should be a deliberate, commented
decision, not the default.

### 2. Real names

The transpiler kept the upstream C# identifiers. Rename everything to what it
means in this editor's vocabulary.

- `RenderX`, `RenderY` → `x`, `y`.
- `RenderX2`, `RenderY2` → whatever the second point actually is: `endX`,
  `targetY`, `stalkY`.
- `img`, `img2` → `body`, `arrow`, `shell`.
- `l`, `m`, `sz`, `w`, `s` → `column`, `row`, `scale`, `segments`.
- The `images` table keys are upstream resource file names (`images.BomOmb`,
  `images.MovingPlatformSection`). Keep them matching the PNG filenames — they
  are the link back to the asset — but a short local alias at the top of
  `render` is fine when a name is used repeatedly.

### 3. Delete the scaffolding

Every transpiled `render` opens with a block of locals the C# needed and the Lua
does not:

```lua
local d = actor.data
local RenderX, RenderY = actor.pixelX, actor.pixelY
local RenderX2, RenderY2 = RenderX, RenderY
local width, height = 0, 0
local img, img2
local drawn = false
```

Keep only what the rewritten function uses. In particular:

- `local drawn = false` followed by `drawn = ... or drawn` on every line is the
  transpiler tracking a return value. When an actor always draws, just
  `return renderer:draw(...)` on the last call.
- `RenderX = (actor.pixelX / 16) * 16` is a round trip through the tile grid that
  the transpiler emitted for the C# `X * 16`. It is just `actor.pixelX`.
- `for l = 0, (n - 1) - 1 do` is a C# `for (l = 0; l < n - 1; l++)`. Write
  `for i = 1, n - 2 do`, or restructure the loop so the bounds read naturally.
- Redundant parentheses: `x = x + (8)`, `(actor.fields.tiles_to_move) * 16`.

### 4. Bounds and render should share their geometry

The transpiler emitted `grabBounds` and `render` independently, so both recompute
the same box. Where they agree, compute it once:

```lua
local function grabBounds(actor)
    local length = actor.fields.platform_width
    return { x = actor.pixelX, y = actor.pixelY, width = 16 * (length + 2), height = 16 }
end

local function render(actor, renderer)
    local box = grabBounds(actor)
    ...
end
```

### 5. Prefer `renderer:draw` over `gfx.blit`

`gfx.blit` exists so 194 generated files did not each have to restate top-left
placement and transform carrying. A hand-written actor usually knows its own
geometry and should call `renderer:draw(image, { x = ..., y = ..., ... })`
directly — compare `data/actors/025-pipe-piranha-plant-facing-right.lua`, which
does its own quarter-turn arithmetic, with what the transpiler produces.

Use `gfx.transform` only when the transform genuinely varies at runtime and is
decided away from the draw call. If a `render` picks between four rotations,
consider four explicit `renderer:draw` calls in the branches instead.

### 6. Comments that carry information

Keep the `-- Ported from NSMB Editor 5's NSMBStageObj.cs case N.` header — it is
the provenance record and `docs/licenses.md` depends on it being there.

Add a comment where the drawing encodes something non-obvious: why an actor's
box is offset from its position, why a value is clamped, what the line or ellipse
represents. Do not narrate the code.

## Do not change behaviour

The rewrite is cosmetic plus the field additions. The drawing must come out
pixel-identical.

- Run `cmake --build build && ctest --test-dir build` after each file. The
  catalog test loads every actor and runs its bounds and render across five
  settings values; a rewrite that throws or produces an out-of-range box fails
  there.
- Watch integer division. The C# was integer arithmetic throughout, so the
  transpiler emitted `//`. Changing a `//` to `/` changes the result.
- Watch operator precedence when you collapse expressions.
- `actor.fields.<key>` for a `checkboxValue` field is a **boolean**, not a
  number. `if actor.fields.smaller then`, never `== 1`.
- Reading an undeclared field key raises. That is deliberate; it is how a typo
  surfaces.

## Not in scope

Ten actors have no `render` because their drawing needs context an actor script
does not get: 206, 243, 245 and 247 need the level's tileset; 274 needs other
actors' positions; 277, 278, 279 and 281 need gradients and level-wide liquid
fills; 228 and 230 need arc strokes and arbitrary-angle rotation. Leave them
alone. Actors 21, 23-26, 28, 29, 30, 60, 77 and 98 are hand-written already and
are the reference for the house style.

The 89 actors with a `grabBounds` and no `render` are correct as they stand — the
old editor draws nothing for them either, so the numbered marker is the right
result. They only need checking against rules 1 and 3 if they read `actor.data`.

## Suggested order

Start with the smallest files, which are mostly a single `gfx.blit` and a
constant box, to settle the house style. Then the loop-and-branch ones. Leave the
handful over 60 lines (49, 65, 172, 189, 190, 193, 195, 234, 235, 285, 286) for
last, when the conventions are established.
