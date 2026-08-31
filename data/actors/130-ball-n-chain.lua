-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 130,
    name = "Ball 'n' Chain",
    category = "Enemies - Castle",
    color = "#535967",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "AKA: Flail, pendulum...",
    grabBounds = { x = 4, y = -11, width = 96, height = 39 },
    renderBounds = { x = 4, y = -11, width = 96, height = 39 },
    sourceFiles = {
        "/enemy/I_tekkyu.nsbmd",
    },
    fields = {
        {
            key = "rotation",
            label = "Rotation",
            bitOffset = 4,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Counterclockwise" },
                { value = 1, label = "Clockwise" },
            },
        },
        {
            key = "starting_position",
            label = "Starting position",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Right" },
                { value = 1, label = "Up" },
                { value = 2, label = "Left" },
                { value = 3, label = "Down" },
            },
        },
    },
    model = readModel("I_tekkyu.nsbmd"),
}
