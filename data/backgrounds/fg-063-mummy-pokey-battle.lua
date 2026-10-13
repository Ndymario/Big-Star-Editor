-- BG_nsc/d_2d_I_M_free_sabaku_boss_UR_nsc.bin
-- StageLayout::bgAnmSabakuBoss.
return {
    layer = "foreground",
    id = 63,
    name = "Mummy-Pokey battle",
    tiles = {
        file = "BG_ncg/d_2d_TEN_I_sabaku_boss_ncg.bin",
        interval = 5,
        frames = { 40, 44, 48, 52, 160, 164, 168, 172 },
        copies = {
            { layer = "foreground", to = 288, count = 4, from = 0 },
            { layer = "foreground", to = 292, count = 4, from = 16 },
            { layer = "foreground", to = 296, count = 4, from = 32 },
        },
    },
}
