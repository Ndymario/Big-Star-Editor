-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 280,
    name = "Jungle FG Effect (Unused Actor)",
    category = "Effects and decorations",
    color = "#5b9b86",
    shape = "box",
    known = true,
    complete = true,
    notes = "* The width of the effect is 15 blocks right from the sprite's place and height is 3 blocks down from the sprite's place and i.e. it is 16x4. The image is repeated sideways\n\n* The effect is above most things.\n\n* Only the first sprite placed defines the where the effect is located.\n\n* The X position of the sprite does not affect the X or Y position of the effect. The Y position does affect the Y position of the effect.\n",
    grabBounds = { x = 0, y = 0, width = 256, height = 64 },
    renderBounds = { x = 0, y = 0, width = 256, height = 64 },
    sourceFiles = {
        "/polygon_unit/W_junglekusa_10.nsbtx",
    },
    fields = {
        {
            key = "swimmable",
            label = "Swimmable",
            bitOffset = 24,
            bitWidth = 8,
            checkboxValue = 1,
        },
        {
            key = "scroll_up_down",
            label = "Scroll Up/Down",
            bitOffset = 0,
            bitWidth = 8,
            notes = "If swimmable, the image doesn't move. If negative value it scrolls down, if positive value it scrolls up.",
            signed = true,
        },
    },
    model = readModel("W_junglekusa_10.nsbtx"),
}
