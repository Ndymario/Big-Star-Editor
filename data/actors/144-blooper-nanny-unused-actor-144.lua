-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 144,
    name = "Blooper Nanny (unused actor 144)",
    category = "Enemies - Water",
    color = "#3f7fc4",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "\"Only someone with direct access to the database can proceed this modification : Class ID is actually 144, not 2 !\"\nThis one doesn't shoot out baby bloopers.",
    grabBounds = { x = 0, y = -16, width = 59, height = 32 },
    renderBounds = { x = 0, y = -16, width = 59, height = 32 },
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
