-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 232,
    name = "Tightrope",
    category = "Platforms and ropes",
    color = "#4f86c6",
    shape = "box",
    known = true,
    complete = true,
    notes = "Usable underwater, but may glitch if the rope part of the sprite is allowed to touch a solid tile while Mario/Luigi walks or stands on it. ",
    grabBounds = { x = -8, y = 0, width = 16, height = 16 },
    renderBounds = { x = -8, y = 0, width = 16, height = 16 },
    sourceFiles = {
        "/enemy/rope.nsbtx",
        "/enemy/rope_end.nsbmd",
    },
    fields = {
        {
            key = "length",
            label = "Length",
            bitOffset = 28,
            bitWidth = 4,
            notes = "If both length and height are set, it will be diagonal.",
        },
        {
            key = "height",
            label = "Height",
            bitOffset = 24,
            bitWidth = 4,
            signed = true,
        },
    },
    model = readModel("rope_end.nsbmd"),
}
