-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 91,
    name = "Snailicorn",
    category = "Enemies - Snow",
    color = "#65a9c7",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "Pushes the player, does collision damage from the front. To kill it, you can either push it off a cliff, use a star, or use a mega mushroom.",
    grabBounds = { x = 0, y = -16, width = 30, height = 32 },
    renderBounds = { x = 0, y = -16, width = 30, height = 32 },
    sourceFiles = {
        "/enemy/donketu.nsbmd",
    },
    model = readModel("donketu.nsbmd"),
}
