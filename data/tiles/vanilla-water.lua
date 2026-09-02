-- Water surfaces, which also keep their own Map16 underneath.

return {
    name = "vanilla.water",
    replaces = false,
    blocks = {
        [41] = { "polygon_unit/I_mizu.nsbtx", "I_mizu_suichu" },
        [147] = { "polygon_unit/I_mizu.nsbtx", "I_mizu_suimen" },
        [148] = { "polygon_unit/I_mizu.nsbtx", "I_mizu_suichu" },
    },
}
