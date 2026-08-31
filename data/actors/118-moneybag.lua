-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 118,
    name = "Moneybag",
    category = "Enemies - Overground",
    color = "#b5651d",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "A greedy bag that hides a lot of coins. Kill it to receive a 1UP. But it takes multiple hits to do so!",
    grabBounds = { x = 0, y = -5, width = 23, height = 21 },
    renderBounds = { x = 0, y = -5, width = 23, height = 21 },
    sourceFiles = {
        "/enemy/gamaguchi.nsbmd",
    },
    model = readModel("gamaguchi.nsbmd"),
}
