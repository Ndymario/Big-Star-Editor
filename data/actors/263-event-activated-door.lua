-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 263,
    name = "Event Activated Door",
    category = "Event controllers",
    color = "#d4a72c",
    shape = "diamond",
    known = true,
    complete = true,
    grabBounds = { x = 0, y = -32, width = 32, height = 48 },
    renderBounds = { x = 0, y = -32, width = 32, height = 48 },
    fields = {
        {
            key = "input_id",
            label = "Input ID",
            bitOffset = 32,
            bitWidth = 8,
        },
        {
            key = "dest_area",
            label = "Dest. area",
            bitOffset = 8,
            bitWidth = 4,
        },
        {
            key = "dest_entrance",
            label = "Dest. entrance",
            bitOffset = 0,
            bitWidth = 8,
        },
    },
}
