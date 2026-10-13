-- BG_nsc/d_2d_I_M_free_koopa_shiro_UR_nsc.bin
-- StageLayout::bgAnmYakata.
return {
    layer = "foreground",
    id = 45,
    name = "Final Castle",
    tiles = {
        file = "BG_ncg/d_2d_TEN_I_koopa_shiro_ncg.bin",
        interval = 5,
        frames = { 0, 4, 8, 12, 80, 84, 88, 92 },
        copies = {
            { layer = "foreground", to = 256, count = 4, from = 0 },
            { layer = "foreground", to = 288, count = 4, from = 16 },
            { layer = "foreground", to = 260, count = 4, from = 32 },
            { layer = "foreground", to = 292, count = 4, from = 48 },
            { layer = "foreground", to = 264, count = 4, from = 64 },
        },
    },
}
