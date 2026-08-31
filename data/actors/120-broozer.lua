-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 120,
    name = "Broozer",
    category = "Enemies - Ghost House",
    color = "#7b5aa6",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "The punching ghost. It can break breakable blocks. It's sound effects are in the ghost house music. Uses the tile god actor to destroy blocks, for some reason. < Is that last bit actually true? I didn't see any tile god listed when using the sprite info printer.[Reply] So, delete it!",
    grabBounds = { x = 0, y = -15, width = 45, height = 31 },
    renderBounds = { x = 0, y = -15, width = 45, height = 31 },
    sourceFiles = {
        "/enemy/crasher.nsbmd",
    },
    model = readModel("crasher.nsbmd"),
}
