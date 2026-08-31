-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 135,
    name = "Cheepskipper",
    category = "Bosses and cutscenes",
    color = "#b84b4b",
    shape = "ellipse",
    known = true,
    complete = true,
    grabBounds = { x = 22, y = -29, width = 59, height = 48 },
    renderBounds = { x = 22, y = -29, width = 59, height = 48 },
    sourceFiles = {
        "/enemy/bosspuku.nsbmd",
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
    model = readModel("bosspuku.nsbmd"),
}
