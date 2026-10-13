-- BG_nsc/d_2d_I_M_back_yakata_nise_UR_nsc.bin
-- StageLayout::fgAnmYakata, which writes into the foreground's tiles: the two layers share one block of them.
return {
    layer = "background",
    id = 58,
    name = "Light grey castle (W8 castle 1)",
    tiles = {
        file = "BG_ncg/d_2d_TEN_I_yakata_nise_ncg.bin",
        interval = 5,
        frames = { 40, 44, 48, 52, 160, 164, 168, 172 },
        copies = {
            { layer = "foreground", to = 14, count = 4, from = 0 },
            { layer = "foreground", to = 46, count = 4, from = 16 },
            { layer = "foreground", to = 78, count = 4, from = 32 },
        },
    },
}
