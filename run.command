#!/bin/sh
# Runs Big Star Editor from source on macOS when double-clicked in Finder, which opens .command files in Terminal.
# Everything is done by run.sh, next to this file.
exec "$(dirname "$0")/run.sh" "$@"
