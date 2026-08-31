-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 93,
    name = "Boss Key",
    category = "Bosses and cutscenes",
    color = "#b84b4b",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "Spawned by boss(related) actors.",
    grabBounds = { x = -20, y = -15, width = 25, height = 31 },
    renderBounds = { x = -20, y = -15, width = 25, height = 31 },
    sourceFiles = {
        "/enemy/koopa_key.nsbmd",
    },
    fields = {
        {
            key = "blocks_horiz_scroll",
            label = "Blocks horiz. scroll",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Not necessarily useful, (and slightly glichy) but it scrolls the camera the defined amount of pixels. high numbers crash the game",
            checkboxValue = 1,
        },
    },
    model = readModel("koopa_key.nsbmd"),
}
