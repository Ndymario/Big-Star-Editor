-- The unused three-wide pillar, in the same tilesets as the fences.

return {
    name = "vanilla.pillar",
    tilesets = { 0x01, 0x27, 0x29, 0x2d, 0x2f, 0x31, 0x3a },
    blocks = {
        [789] = { "polygon_unit/I_hashira_lu.nsbtx", "I_hashira_lu" },
        [790] = { "polygon_unit/I_hashira_cu.nsbtx", "I_hashira_cu" },
        [791] = { "polygon_unit/I_hashira_ru.nsbtx", "I_hashira_ru" },
        [792] = { "polygon_unit/I_hashira_l.nsbtx", "I_hashira_l" },
        [793] = { "polygon_unit/I_hashira_c.nsbtx", "I_hashira_c" },
        [794] = { "polygon_unit/I_hashira_r.nsbtx", "I_hashira_r" },
        [795] = { "polygon_unit/I_hashira_ld.nsbtx", "I_hashira_ld" },
        [796] = { "polygon_unit/I_hashira_cd.nsbtx", "I_hashira_cd" },
        [797] = { "polygon_unit/I_hashira_rd.nsbtx", "I_hashira_rd" },
    },
}
