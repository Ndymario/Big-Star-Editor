-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 136,
    name = "Jumping Cheep Cheep",
    category = "Enemies - Water",
    color = "#3f7fc4",
    shape = "ellipse",
    known = true,
    complete = false,
    notes = "Funny (but useful) Glitch:\nIf you put a Green Cheep Cheep on the same screen as one of these, it becomes green!",
    grabBounds = { x = 0, y = 0, width = 21, height = 16 },
    renderBounds = { x = 0, y = 0, width = 21, height = 16 },
    fields = {
        {
            key = "unknown_10",
            label = "Unknown 10",
            bitOffset = 4,
            bitWidth = 4,
            notes = "1=Stays on screen and only jumps when the boss tells it to. Used in Cheepskipper battle. 2=Also used for some of them the in Cheepskipper battle, purpose unkonwn",
            choices = {
                { value = 0, label = "Normal" },
                { value = 1, label = "Unknown 1" },
                { value = 2, label = "Unknown 2" },
            },
        },
        {
            key = "unknown_11",
            label = "Unknown 11",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Set to 1 for some of them in the Cheepskipper battle.",
        },
    },
}
