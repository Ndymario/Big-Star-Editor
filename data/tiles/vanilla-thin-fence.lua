-- The thin fences.
--
-- Two things happen here that no other block does: 0x311-0x314 are rewritten to
-- the block four below rather than to a texture, and 0x320-0x323 and 0x32c-0x330
-- are blanked with no quad behind them at all.

return {
    name = "vanilla.thin-fence",
    tilesets = { 0x01, 0x27, 0x29, 0x2d, 0x2f, 0x31, 0x3a },
    blocks = {
        [785] = { remap = 781 },
        [786] = { remap = 782 },
        [787] = { remap = 783 },
        [788] = { remap = 784 },
        [800] = {},
        [801] = {},
        [802] = {},
        [803] = {},
        [812] = {},
        [813] = {},
        [814] = {},
        [815] = {},
        [816] = {},
    },
}
