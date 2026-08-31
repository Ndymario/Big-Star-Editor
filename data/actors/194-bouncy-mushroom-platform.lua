-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 194,
    name = "Bouncy Mushroom Platform",
    category = "Platforms and ropes",
    color = "#4f86c6",
    shape = "box",
    known = true,
    complete = true,
    notes = "No problems with underwater players bouncing on it, but doesn't appear to have special underwater behaviour.",
    grabBounds = { x = -40, y = 0, width = 96, height = 32 },
    renderBounds = { x = -40, y = 0, width = 96, height = 32 },
    sourceFiles = {
        "/enemy/I_kinoko_ashib_t.nsbtx",
        "/enemy/I_kinoko_kuki_t.nsbtx",
        "/enemy/I_kinoko_kuki_tl.nsbtx",
    },
    fields = {
        {
            key = "stalk_height",
            label = "Stalk height",
            bitOffset = 0,
            bitWidth = 4,
        },
        {
            key = "shift_position",
            label = "Shift position",
            bitOffset = 4,
            bitWidth = 4,
            choices = {
                { value = 0, label = "No shifting" },
                { value = 1, label = "8 pixels left" },
                { value = 2, label = "8 pixels right" },
            },
        },
    },
    model = readModel("I_kinoko_ashib_t.nsbtx"),
}
