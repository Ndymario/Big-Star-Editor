-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 264,
    name = "Toad House block",
    category = "Bosses and cutscenes",
    color = "#b84b4b",
    shape = "ellipse",
    known = true,
    complete = true,
    grabBounds = { x = 0, y = 0, width = 16, height = 16 },
    renderBounds = { x = 0, y = 0, width = 16, height = 16 },
    sourceFiles = {
        "/obj/I_minigame_block_ncg.bin",
    },
    fields = {
        {
            key = "bonus_game",
            label = "Bonus game",
            bitOffset = 28,
            bitWidth = 4,
            choices = {
                { value = 0, label = "1-Up Minigame" },
                { value = 1, label = "Background Chooser" },
            },
        },
        {
            key = "unknown_7",
            label = "Unknown 7",
            bitOffset = 16,
            bitWidth = 4,
            notes = "1 is used when big roulette block is spawned by Toadsworth",
        },
        {
            key = "shift_position",
            label = "Shift position",
            bitOffset = 4,
            bitWidth = 4,
            choices = {
                { value = 0, label = "No shifting" },
                { value = 1, label = "8 pixels right" },
            },
        },
        {
            key = "block_id",
            label = "Block ID",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 0, label = "1" },
                { value = 1, label = "2" },
                { value = 2, label = "3" },
                { value = 3, label = "4" },
                { value = 4, label = "5" },
                { value = 5, label = "6" },
            },
        },
    },
}
