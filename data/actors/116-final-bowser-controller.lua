-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 116,
    name = "Final Bowser controller",
    category = "Bosses and cutscenes",
    color = "#b84b4b",
    shape = "ellipse",
    known = true,
    complete = false,
    notes = "Spawns Bowser Jr. and Big Bowser. See those sprites for more info. Also makes peach say \"Help!\" When on screen.",
    grabBounds = { x = -125, y = -28, width = 162, height = 43 },
    renderBounds = { x = -125, y = -28, width = 162, height = 43 },
    sourceFiles = {
        "/enemy/pot.nsbmd",
    },
    model = readModel("pot.nsbmd"),
}
