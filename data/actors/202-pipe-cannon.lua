-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 202,
    name = "Pipe Cannon",
    category = "Platforms and ropes",
    color = "#4f86c6",
    shape = "box",
    known = true,
    complete = true,
    grabBounds = { x = 1, y = -49, width = 55, height = 65 },
    renderBounds = { x = 1, y = -49, width = 55, height = 65 },
    sourceFiles = {
        "/polygon_unit/I_dokan.nsbtx (SOMETHING_earthen pipe)",
    },
    fields = {
        {
            key = "angle_force",
            label = "Angle / Force",
            bitOffset = 0,
            bitWidth = 4,
            notes = "1 is unused.",
            choices = {
                { value = 0, label = "Far right" },
                { value = 1, label = "Far left" },
                { value = 2, label = "Medium right" },
                { value = 3, label = "Medium left" },
                { value = 4, label = "Straight up" },
                { value = 5, label = "Very far right" },
            },
        },
    },
    model = readModel("I_dokan.nsbtx"),
}
