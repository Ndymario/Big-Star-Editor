-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 128,
    name = "Mummipokey",
    category = "Bosses and cutscenes",
    color = "#b84b4b",
    shape = "ellipse",
    known = true,
    complete = true,
    grabBounds = { x = -256, y = 16, width = 512, height = 95 },
    renderBounds = { x = -256, y = 16, width = 512, height = 95 },
    sourceFiles = {
        "/enemy/I_sandoboss_ishi.nsbmd",
        "/enemy/I_sandobossjimen.nsbmd",
        "/enemy/sand_worm.nsbmd",
    },
    fields = {
        {
            key = "trigger_id",
            label = "Trigger ID",
            bitOffset = 32,
            bitWidth = 8,
        },
    },
    model = readModel("I_sandoboss_ishi.nsbmd"),
}
