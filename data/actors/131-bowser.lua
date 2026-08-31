-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 131,
    name = "Bowser",
    category = "Bosses and cutscenes",
    color = "#b84b4b",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "- Can start moving without being activated in some circumstances [desmume glitch?]\n- After roaring it allows Mario to move after boss activator makes him stop.",
    grabBounds = { x = -41, y = -38, width = 76, height = 55 },
    renderBounds = { x = -41, y = -38, width = 76, height = 55 },
    sourceFiles = {
        "/enemy/koopa_new.nsbmd",
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
    model = readModel("koopa_new.nsbmd"),
}
