-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 225,
    name = "Tweester (Tornado)",
    category = "Enemies - Desert",
    color = "#c9833b",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "This tornado, may or may not be the same thing as a Tweester.\n\nSlings you up in the air, after which you can Drill Stomp.",
    grabBounds = { x = -88, y = -130, width = 191, height = 150 },
    renderBounds = { x = -88, y = -130, width = 191, height = 150 },
    sourceFiles = {
        "/enemy/sand_tornado.nsbmd",
    },
    model = readModel("sand_tornado.nsbmd"),
}
