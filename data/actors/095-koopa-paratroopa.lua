-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 95,
    name = "Koopa Paratroopa",
    category = "Enemies - Overground",
    color = "#b5651d",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "Unused behaviours:\n- Has water behaviour when Direction is set to Jumping [may be used, or koopa leftover]",
    grabBounds = { x = 0, y = -16, width = 19, height = 32 },
    renderBounds = { x = 0, y = -16, width = 19, height = 32 },
    sourceFiles = {
        "/obj/I_nokonoko256_ncg.bin",
        "/obj/I_enemy_B_ncl.bin",
        "If you want to change colors in the Koopa Troopa's pallette, open 'I_enemy_B_ncl.bin', because that's the palette which belongs to it. When you edit the colors with 'I_enemy_W_ncl' for example, nothing will change in NSMB.",
    },
    fields = {
        {
            key = "starting_direction",
            label = "Starting direction",
            bitOffset = 12,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Down/Left" },
                { value = 1, label = "Up/Right" },
            },
        },
        {
            key = "starting_position",
            label = "Starting position",
            bitOffset = 8,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Midway" },
                { value = 1, label = "Start Bottom-Left" },
                { value = 2, label = "Start Top-Right" },
            },
        },
        {
            key = "direction",
            label = "Direction",
            bitOffset = 4,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Jumping" },
                { value = 1, label = "Left-right" },
                { value = 2, label = "Up-down" },
            },
        },
        {
            key = "color",
            label = "Color",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Red Koopas stays on ledges.",
            choices = {
                { value = 0, label = "Green" },
                { value = 1, label = "Red" },
                { value = 2, label = "Blue" },
                { value = 3, label = "Blue (stays on ledges)" },
            },
        },
    },
    graphics = readGraphics("I_nokonoko256_ncg.bin"),
    palette = readPalette("I_enemy_B_ncl.bin"),
}
