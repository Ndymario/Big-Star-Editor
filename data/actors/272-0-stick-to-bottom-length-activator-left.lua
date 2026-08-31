-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 272,
    name = "0 Stick to bottom length activator left",
    category = "Camera controllers",
    color = "#3f9ca8",
    shape = "box",
    known = true,
    complete = true,
    notes = "The DS screen is 12 tiles high. When this is used as a top limit, it will control the tiles below the icon. As a bottom limit, it controls tiles above.",
    grabBounds = { x = 0, y = 0, width = 16, height = 16 },
    renderBounds = { x = 0, y = 0, width = 16, height = 16 },
    fields = {
        {
            key = "type",
            label = "Type",
            bitOffset = 28,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Bottom limit" },
                { value = 1, label = "Top limit" },
            },
        },
        {
            key = "pix_past_limit",
            label = "Pix. past limit",
            bitOffset = 0,
            bitWidth = 8,
            notes = "Pixels able to be seen past limit. This will shift 12 tile screen down from icon position.",
        },
        {
            key = "death_below_limit",
            label = "Death below limit",
            bitOffset = 12,
            bitWidth = 4,
            notes = "Must be empty map block, no tiles below. Mario image must fall out of screen.",
            checkboxValue = 1,
        },
    },
}
