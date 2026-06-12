# License considerations

This repository is currently GPL-3.0 because the root `LICENSE` is GPL version 3. New code in this milestone is written from scratch for this repository.

## Old NSMB Editor

The current NSMB Editor repository contains GPL-3.0-or-later notices. Copying or translating source code from it would require preserving compatible GPL obligations and attribution. M0 does not copy or line-by-line port code; it records behavioral observations only.

## Fireflower

Fireflower includes a license file and useful extraction/build tools. M0 does not vendor Fireflower or copy implementation code. It only defines service interfaces/wrappers that can invoke external `nds-extract` and `nds-build` executables supplied by the user.

## NCPatcher

NCPatcher is an external C++ tool. M0 does not vendor it. The integration is a narrow external-process boundary that expects a user-installed `ncpatcher` executable and a project-local `ncpatcher.json`.

## NSMB Code Reference and Code Template

These repositories are references for symbols and workflows. They can involve SDK/toolchain assumptions and generated project files. Do not copy Nintendo SDK headers, Nitro SDK contents, or copyrighted game data. If future work copies template text or code, verify the license first and document attribution.

## Nintendo ROM data and SDK content

No Nintendo ROM, SDK, Nitro SDK, extracted assets, or game data may be committed. Tests must use synthetic fixtures only.

## Future dependencies

Before adding third-party code under `third_party/`, document the license, source URL, version/commit, why vendoring is necessary, update policy, and compatibility with GPL-3.0 distribution.
