-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 129,
    name = "Lakithunder",
    category = "Bosses and cutscenes",
    color = "#b84b4b",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "Swoops down on the player, and tries to strike the player with Lightning as well. Can only be spawned with a Castle Boss Controller.",
    grabBounds = { x = 125, y = -15, width = 37, height = 45 },
    renderBounds = { x = 125, y = -15, width = 37, height = 45 },
    sourceFiles = {
        "/enemy/thunder_jugem.nsbmd",
    },
    fields = {
        {
            key = "trigger_id",
            label = "Trigger ID",
            bitOffset = 32,
            bitWidth = 8,
        },
    },
    model = readModel("thunder_jugem.nsbmd"),
}
