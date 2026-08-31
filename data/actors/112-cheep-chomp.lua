-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 112,
    name = "Cheep Chomp",
    category = "Enemies - Water",
    color = "#3f7fc4",
    shape = "ellipse",
    known = true,
    complete = true,
    grabBounds = { x = 0, y = 0, width = 60, height = 47 },
    renderBounds = { x = 0, y = 0, width = 60, height = 47 },
    sourceFiles = {
        "/enemy/bakubaku.nsbmd",
    },
    fields = {
        {
            key = "eat",
            label = "Eat",
            bitOffset = 40,
            bitWidth = 4,
            notes = "Do not shoot fire when it is about to eat you or the game will block, it is one of the game's fails",
        },
    },
    model = readModel("bakubaku.nsbmd"),
}
