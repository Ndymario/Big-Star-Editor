-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 283,
    name = "Touching ground vertical scroll stop left ",
    category = "Camera controllers",
    color = "#3f9ca8",
    shape = "box",
    known = true,
    complete = true,
    notes = "This sprite seems to disable all other camera sprites when activated.\n\nIt also changed horizontal scrolling...I think.",
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
            key = "visible_y_pix_past",
            label = "Visible Y pix. past",
            bitOffset = 0,
            bitWidth = 8,
            notes = "Pixels able to be seen past limit.",
        },
        {
            key = "disable_y_pix_past",
            label = "Disable Y pix. past",
            bitOffset = 8,
            bitWidth = 8,
            notes = "Can only be set to a max of 31 pixels. Past that move the sprite.",
        },
    },
}
