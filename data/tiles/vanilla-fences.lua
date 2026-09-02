-- Wire fences, in the tilesets that have them.
--
-- `3dFencesTileIDs @ 020c75b8`, reached only when the level's tileset is one of
-- the castle, tower and boss sets.

return {
    name = "vanilla.fences",
    tilesets = { 0x01, 0x27, 0x29, 0x2d, 0x2f, 0x31, 0x3a },
    blocks = {
        [768] = { "polygon_unit/wire_netting2.nsbtx", "wire_netting2" },
        [769] = { "polygon_unit/wire_netting3.nsbtx", "wire_netting3" },
        [770] = { "polygon_unit/wire_netting4.nsbtx", "wire_netting4" },
        [771] = { "polygon_unit/wire_netting6.nsbtx", "wire_netting6" },
        [772] = { "polygon_unit/wire_netting7.nsbtx", "wire_netting7" },
        [773] = { "polygon_unit/wire_netting9.nsbtx", "wire_netting9" },
        [774] = { "polygon_unit/wire_netting10.nsbtx", "wire_netting10" },
        [775] = { "polygon_unit/wire_netting12.nsbtx", "wire_netting12" },
        [776] = { "polygon_unit/wire_netting13.nsbtx", "wire_netting13" },
        [777] = { "polygon_unit/wire_netting14.nsbtx", "wire_netting14" },
        [778] = { "polygon_unit/wire_netting15.nsbtx", "wire_netting15" },
        [779] = { "polygon_unit/wire_netting17.nsbtx", "wire_netting17" },
        [780] = { "polygon_unit/wire_netting1.nsbtx", "wire_netting1" },
    },
}
