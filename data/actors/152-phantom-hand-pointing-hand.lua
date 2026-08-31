-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 152,
    name = "Phantom Hand (Pointing hand)",
    category = "Enemies - Ghost House",
    color = "#7b5aa6",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "Points at connected Winged ? Blocks.\nAlso points to Unused Invisible Podoboos without connections.",
    grabBounds = { x = 8, y = 0, width = 16, height = 20 },
    renderBounds = { x = 8, y = 0, width = 16, height = 20 },
    sourceFiles = {
        "/enemy/finger.nsbmd",
    },
    fields = {
        {
            key = "phantom_hand_id",
            label = "Phantom Hand ID",
            bitOffset = 0,
            bitWidth = 4,
        },
    },
    model = readModel("finger.nsbmd"),
}
