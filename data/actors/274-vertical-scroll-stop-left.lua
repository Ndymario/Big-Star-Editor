-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 274,
    name = "Vertical scroll stop left",
    category = "Camera controllers",
    color = "#3f9ca8",
    shape = "box",
    known = true,
    complete = true,
    grabBounds = { x = 0, y = 0, width = 16, height = 16 },
    renderBounds = { x = 0, y = 0, width = 16, height = 16 },
    fields = {
        {
            key = "type",
            label = "Type",
            bitOffset = 28,
            bitWidth = 4,
            notes = "Bottom or top of area that camera can enter.",
            choices = {
                { value = 0, label = "Bottom limit" },
                { value = 1, label = "Top limit" },
            },
        },
        {
            key = "shift_up_in_pixels",
            label = "Shift up (in pixels)",
            bitOffset = 0,
            bitWidth = 8,
            notes = "Pixels able to be seen past limit.",
        },
        {
            key = "allow_disable",
            label = "Allow disable",
            bitOffset = 12,
            bitWidth = 4,
            notes = "Allow Mario and camera to go past the limit, if Mario touches it.",
            checkboxValue = 1,
        },
        {
            key = "id",
            label = "ID",
            bitOffset = 8,
            bitWidth = 4,
        },
    },
}
