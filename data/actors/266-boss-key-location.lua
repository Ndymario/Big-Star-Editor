-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 266,
    name = "Boss Key Location",
    category = "Bosses and cutscenes",
    color = "#b84b4b",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "If the boss is defeated, the Boss Battle Key falls on the place where you placed this sprite.\nIf else it falls in a default place?",
    grabBounds = { x = -20, y = -15, width = 25, height = 31 },
    renderBounds = { x = -20, y = -15, width = 25, height = 31 },
    sourceFiles = {
        "/enemy/koopa_key.nsbmd",
    },
    model = readModel("koopa_key.nsbmd"),
}
