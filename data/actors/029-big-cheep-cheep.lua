-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 29,
    name = "Big Cheep Cheep",
    category = "Enemies - Water",
    color = "#3f7fc4",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "Unused behaviours:\n- Can be defeated by ground pound for 200pts, with no effect [may be default behaviour]",
    grabBounds = { x = -20, y = -32, width = 63, height = 52 },
    renderBounds = { x = -20, y = -32, width = 63, height = 52 },
    sourceFiles = {
        "/enemy/big_pukupuku.nsbmd",
    },
    fields = {
        {
            key = "slower",
            label = "Slower",
            bitOffset = 12,
            bitWidth = 4,
            notes = "Unused.",
            checkboxValue = 1,
        },
        {
            key = "green_chases_mario",
            label = "Green (chases Mario)",
            bitOffset = 0,
            bitWidth = 4,
            checkboxValue = 1,
        },
    },
    model = readModel("big_pukupuku.nsbmd"),
}
