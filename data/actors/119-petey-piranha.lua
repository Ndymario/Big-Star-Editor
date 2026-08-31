-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 119,
    name = "Petey Piranha",
    category = "Bosses and cutscenes",
    color = "#b84b4b",
    shape = "ellipse",
    known = true,
    complete = true,
    grabBounds = { x = -16, y = -56, width = 108, height = 75 },
    renderBounds = { x = -16, y = -56, width = 108, height = 75 },
    sourceFiles = {
        "/enemy/boss_packun.nsbmd",
    },
    fields = {
        {
            key = "trigger_id_1",
            label = "Trigger ID 1",
            bitOffset = 32,
            bitWidth = 8,
        },
        {
            key = "trigger_id_2",
            label = "Trigger ID 2",
            bitOffset = 40,
            bitWidth = 8,
        },
    },
    model = readModel("boss_packun.nsbmd"),
}
