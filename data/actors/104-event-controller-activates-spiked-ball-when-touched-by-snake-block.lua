-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 104,
    name = "Event controller - activates Spiked Ball when touched by Snake Block",
    category = "Other",
    color = "#eeB24b",
    shape = "diamond",
    known = true,
    complete = true,
    notes = "Activates a Spiked Ball when a Snake Block pass over it.",
    grabBounds = { x = 0, y = 0, width = 16, height = 16 },
    renderBounds = { x = 0, y = 0, width = 16, height = 16 },
    fields = {
        {
            key = "spiked_ball_id",
            label = "Spiked Ball ID",
            bitOffset = 0,
            bitWidth = 4,
        },
    },
}
