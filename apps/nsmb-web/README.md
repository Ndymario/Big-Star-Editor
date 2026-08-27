# Big Star Web

This directory builds a small, dependency-free browser level editor over the
portable Big Star backend. Level bytes cross an explicit C ABI; JavaScript,
DOM, file-picker, download, and browser-storage concepts stay outside `libs/`.

The current web surface opens a user-provided course/BGDat pair, displays a
schematic canvas, edits objects, sprites, entrances, views, and zones, supports
add/delete and undo/redo, and downloads the loss-preserving serialized pair.
Path nodes, object banks, and resource references are present in the snapshot
contract but are not yet editing surfaces in this deliberately narrow shell.
Files remain local to the browser and are never uploaded.

## Build and run

Install or activate Emscripten, then run from the repository root:

```sh
emcmake cmake -S . -B build-wasm \
  -DBSE_BUILD_TESTS=OFF -DBSE_BUILD_QT=OFF -DCMAKE_BUILD_TYPE=Release
cmake --build build-wasm --target nsmb-web
python3 -m http.server --directory build-wasm/apps/nsmb-web
```

Open `http://127.0.0.1:8000/`. The output directory contains `index.html`,
`app.js`, `style.css`, `big-star.js`, and `big-star.wasm`. Serve it over HTTP;
opening the HTML directly does not provide a reliable WebAssembly fetch origin.

For a normal native build, `nsmb-web-api-tests` exercises the same adapter
without a browser.

## C ABI contract

[`include/big_star_web.h`](include/big_star_web.h) is the browser-neutral ABI.
`bse_api_version()` currently returns `0x00010000` (1.0). Callers open a session
from course/BGDat byte spans, consume a named JSON snapshot, submit mutations,
and serialize before reading the two output buffers. Returned pointers remain
owned by Big Star and have the lifetimes documented in the header. Every entry
point catches exceptions and reports failures through `bse_last_error()`.

Collection and zero-based field order for `bse_level_set`:

| Collection | Fields |
|---|---|
| Objects | `id`, `slot`, `x`, `y`, `width`, `height` |
| Sprites | `id`, `x`, `y`, `event0`, `event1`, `settings` |
| Entrances | `x`, `y`, `cameraX`, `cameraY`, `id`, `targetArea`, `targetStage`, `targetGroup`, `targetEntrance`, `cameraZoom`, `type`, `flags`, `unknown10`, `unknown11`, `viewId`, `worldmapNode` |
| Views | `x`, `y`, `width`, `height`, `id`, `camera`, `bgm`, `background`, `tileset`, `foreground`, `light`, `progressPath` |
| Zones | `x`, `y`, `width`, `height`, `id`, `unknown9`, `unknownA`, `unknownB` |
| Paths / progress paths | `id`, `unknown` (nodes are snapshot-only) |
| Object banks | one value at field `0` |
| Resource references | `tileset`, `backgroundGraphics`, `backgroundScreen`, `foregroundGraphics`, `foregroundScreen` |

The web target intentionally excludes ROM import, module/project filesystem
access, external patchers, and browser persistence. Those workflows belong to
the desktop/CLI frontends or to a future adapter with an explicit contract.
