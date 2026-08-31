-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 268,
    name = "Respawnable Hidden Block",
    category = "Other",
    color = "#eeB24b",
    shape = "diamond",
    known = true,
    complete = true,
    notes = "Respawns when destroyed.",
    grabBounds = { x = 0, y = 0, width = 16, height = 16 },
    renderBounds = { x = 0, y = 0, width = 16, height = 16 },
    fields = {
        {
            key = "horizontal_detection_for_solid_tile",
            label = "Horizontal detection for solid tile",
            bitOffset = 16,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Left" },
                { value = 1, label = "Right" },
            },
        },
        {
            key = "horizontal_distance_from_solid_tile",
            label = "Horizontal distance from solid tile",
            bitOffset = 12,
            bitWidth = 4,
        },
        {
            key = "vertical_detection_for_solid_tile",
            label = "Vertical detection for solid tile",
            bitOffset = 8,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Up" },
                { value = 1, label = "Down" },
            },
        },
        {
            key = "vertical_distance_from_solid_tile",
            label = "Vertical distance from solid tile",
            bitOffset = 4,
            bitWidth = 4,
        },
    },
}
