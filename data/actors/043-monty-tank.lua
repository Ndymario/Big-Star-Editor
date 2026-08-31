-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 43,
    name = "Monty Tank",
    category = "Bosses and cutscenes",
    color = "#b84b4b",
    shape = "ellipse",
    known = true,
    complete = true,
    grabBounds = { x = -374, y = -37, width = 72, height = 52 },
    renderBounds = { x = -374, y = -37, width = 72, height = 52 },
    sourceFiles = {
        "/enemy/body_1.nsbmd",
        "/enemy/body_2.nsbmd",
        "/enemy/body_3.nsbmd",
        "/enemy/choropu.nsbmd",
        "/enemy/tire.nsbmd",
    },
    fields = {
        {
            key = "trigger_id",
            label = "Trigger ID",
            bitOffset = 32,
            bitWidth = 8,
        },
    },
    model = readModel("body_1.nsbmd"),
}
