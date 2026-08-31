-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 45,
    name = "Chain Chomp (unused sprite)",
    category = "Enemies - Overground",
    color = "#b5651d",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "Spawned by Chain Chomp/Log. When spawned via a sprite it can be defeated by ground-pounding its head for some reason.",
    grabBounds = { x = -29, y = -28, width = 44, height = 28 },
    renderBounds = { x = -29, y = -28, width = 44, height = 28 },
    sourceFiles = {
        "/enemy/wanwan.nsbmd",
        "/enemy/wanwan_chain.nsbmd",
    },
    model = readModel("wanwan.nsbmd"),
}
