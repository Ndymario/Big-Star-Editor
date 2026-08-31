-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 132,
    name = "Dry Bowser",
    category = "Bosses and cutscenes",
    color = "#b84b4b",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "Regular Bowser seen in 1-Castle, exept throws bones and is immune to Fireballs.",
    grabBounds = { x = -42, y = -40, width = 80, height = 57 },
    renderBounds = { x = -42, y = -40, width = 80, height = 57 },
    sourceFiles = {
        "/enemy/bonekoopa.nsbmd",
    },
    fields = {
        {
            key = "trigger_id",
            label = "Trigger ID",
            bitOffset = 32,
            bitWidth = 8,
        },
        {
            key = "bridge_id",
            label = "Bridge ID",
            bitOffset = 40,
            bitWidth = 8,
        },
    },
    model = readModel("bonekoopa.nsbmd"),
}
