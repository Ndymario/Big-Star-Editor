-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 60,
    name = "Cheep Cheep random spawner",
    category = "Enemies - Water",
    color = "#3f7fc4",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "Let spawn random generated Cheep Cheep in the selected Zone.",
    grabBounds = { x = -1, y = 4, width = 19, height = 18 },
    renderBounds = { x = -1, y = 4, width = 19, height = 18 },
    sourceFiles = {
        "/enemy/pukupuku.nsbmd",
    },
    fields = {
        {
            key = "zone_id",
            label = "Zone ID",
            bitOffset = 8,
            bitWidth = 8,
            notes = "Let spawn random generated Cheep-Cheep in the selected Zone.",
        },
        {
            key = "green_chases_mario",
            label = "Green (chases Mario)",
            bitOffset = 4,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "faster_out_of_water",
            label = "Faster & out of water",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Makes some swim faster and makes them cover whole screen, not just part with water",
            checkboxValue = 1,
        },
    },
    model = readModel("pukupuku.nsbmd"),
}
