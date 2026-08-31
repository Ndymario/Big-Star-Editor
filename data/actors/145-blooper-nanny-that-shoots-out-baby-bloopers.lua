-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 145,
    name = "Blooper Nanny that shoots out Baby Bloopers",
    category = "Enemies - Water",
    color = "#3f7fc4",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "This one shoots out baby bloopers after a rainbow effect",
    grabBounds = { x = -8, y = -32, width = 42, height = 64 },
    renderBounds = { x = -8, y = -32, width = 42, height = 64 },
    sourceFiles = {
        "/enemy/gesso.nsbmd",
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
        {
            key = "avoids_mario",
            label = "Avoids Mario",
            bitOffset = 0,
            bitWidth = 4,
            checkboxValue = 1,
        },
    },
    model = readModel("gesso.nsbmd"),
}
