# M14 WebAssembly manual checklist

Build the web target with Emscripten, serve its output over HTTP, and use a
user-provided course/BGDat pair. No ROM or game asset is packaged with Big Star.

1. Confirm the page reports WebAssembly ABI 1.0 and keeps **Open level** disabled until the module is ready.
2. Select a matching course and BGDat pair. Confirm the page names both files, shows counts, renders the schematic canvas, and never performs a network upload.
3. Switch among Objects, Sprites, Entrances, Views, and Zones. Confirm table counts and named fields match the desktop representation.
4. Change a numeric field, add and delete a row, then exercise Undo and Redo. Confirm the table, canvas, counts, and button states refresh after each command.
5. Enter an out-of-range object ID or slot. Confirm the backend refuses it and presents a readable error instead of corrupting the session.
6. Download both files, reopen the downloaded pair, and confirm the edit survives. Compare untouched records and unknown bytes with the inputs.
7. Open a malformed or mismatched pair. Confirm it is refused without hiding a previously valid browser tab or crashing the WebAssembly runtime.
8. Repeat at narrow and wide viewport sizes with keyboard navigation. Confirm file controls, history controls, entity tabs, table, and canvas remain reachable.

On 2026-08-27, the release WebAssembly build was compiled with Emscripten 6.0.8
and exercised against `J01_1.bin` and `J01_1_bgdat.bin`: 122 objects, 9 sprites,
3 entrances, 1 view, and 0 zones loaded; numeric editing, range refusal,
add/delete, undo/redo, and course serialization/download all succeeded. A
subsequent malformed pair was refused while the valid session remained usable.
