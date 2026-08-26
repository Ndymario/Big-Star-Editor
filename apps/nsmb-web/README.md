# nsmb-web placeholder

The web frontend is intentionally not implemented in M0. The intended boundary is a thin web adapter around the portable C++ backend, most likely via Emscripten/WebAssembly and a small C ABI or explicit binding layer. Browser APIs, DOM types, JavaScript promises, and storage concepts must remain outside `libs/*`.
