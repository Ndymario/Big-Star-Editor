-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 204,
    name = "Corked Pipe + foot pump",
    category = "Other",
    color = "#eeB24b",
    shape = "diamond",
    known = true,
    complete = true,
    grabBounds = { x = 0, y = -56, width = 32, height = 72 },
    renderBounds = { x = 0, y = -56, width = 32, height = 72 },
    sourceFiles = {
        "/enemy/obj_bellows.nsbmd",
        "/enemy/obj_cork.nsbmd",
    },
    fields = {
        {
            key = "destination_area",
            label = "Destination area",
            bitOffset = 20,
            bitWidth = 4,
        },
        {
            key = "destination_entrance",
            label = "Destination entrance",
            bitOffset = 0,
            bitWidth = 4,
        },
        {
            key = "horizontal_distance_between_pump_and_tube",
            label = "Horizontal distance between pump and tube",
            bitOffset = 4,
            bitWidth = 4,
        },
        {
            key = "vertical_distance_between_pump_and_tube",
            label = "Vertical distance between pump and tube",
            bitOffset = 12,
            bitWidth = 4,
        },
        {
            key = "shift_21_tiles_left",
            label = "Shift 21 tiles left",
            bitOffset = 8,
            bitWidth = 4,
            choices = {
                { value = 1, label = "Yes" },
                { value = 0, label = "No" },
            },
        },
        {
            key = "shift_16_tiles_down",
            label = "Shift 16 tiles down",
            bitOffset = 16,
            bitWidth = 4,
            choices = {
                { value = 1, label = "Yes" },
                { value = 0, label = "No" },
            },
        },
    },
    model = readModel("obj_bellows.nsbmd"),
}
