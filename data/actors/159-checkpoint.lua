-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 159,
    name = "Checkpoint",
    category = "Other",
    color = "#eeB24b",
    shape = "diamond",
    known = true,
    complete = false,
    notes = "The midway point. The ID of the Start position needs to be configured in the View's settings.",
    grabBounds = { x = 0, y = 0, width = 22, height = 33 },
    renderBounds = { x = 0, y = 0, width = 22, height = 33 },
    sourceFiles = {
        "/enemy/s_flag.nsbmd",
    },
    fields = {
        {
            key = "unknown_11",
            label = "Unknown 11",
            bitOffset = 0,
            bitWidth = 4,
        },
    },
    model = readModel("s_flag.nsbmd"),
}
