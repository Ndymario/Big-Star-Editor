-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 90,
    name = "Swooper",
    category = "Enemies - Underground",
    color = "#686d76",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "A bat.",
    grabBounds = { x = 0, y = 0, width = 16, height = 18 },
    renderBounds = { x = 0, y = 0, width = 16, height = 18 },
    sourceFiles = {
        "/enemy/basabasa.nsbmd",
        "/enemy/basabasa_wait.nsbmd",
    },
    fields = {
        {
            key = "size",
            label = "Size",
            bitOffset = 0,
            bitWidth = 8,
            notes = "Somewhat buggy when putting values between 2 and 17 inclusive as the sprite and the hitbox become completely disjointed.",
            choices = {
                { value = 0, label = "Small" },
                { value = 1, label = "Big" },
                { value = 32, label = "Small (shift 1 tile down)" },
                { value = 33, label = "Big (shift 2 tiles down)" },
            },
        },
    },
    model = readModel("basabasa_wait.nsbmd"),
}
