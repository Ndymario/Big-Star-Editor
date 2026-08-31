-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 157,
    name = "Spike Bass spawner",
    category = "Enemies - Water",
    color = "#3f7fc4",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "Spawns a Spike Bass into onscreen actor-based liquid if the player is in the zone.\n\nTest: multiple onscreen liquids.",
    grabBounds = { x = -3, y = -10, width = 45, height = 53 },
    renderBounds = { x = -3, y = -10, width = 45, height = 53 },
    sourceFiles = {
        "/enemy/togepuku.nsbmd",
    },
    fields = {
        {
            key = "zone_id",
            label = "Zone ID",
            bitOffset = 8,
            bitWidth = 8,
        },
        {
            key = "use_restricted_movement",
            label = "Use Restricted Movement",
            bitOffset = 4,
            bitWidth = 4,
            notes = "Unused.",
            checkboxValue = 1,
        },
        {
            key = "restricted_movement_distance",
            label = "Restricted Movement Distance",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Unused.",
        },
    },
    model = readModel("togepuku.nsbmd"),
}
