-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 88,
    name = "Fire Bro.",
    category = "Enemies - Overground",
    color = "#b5651d",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "This enemy is similar to the Hammer Bro., but throws fire balls",
    grabBounds = { x = 0, y = -15, width = 18, height = 31 },
    renderBounds = { x = 0, y = -15, width = 18, height = 31 },
    sourceFiles = {
        "/enemy/hammer_bros.nsbmd",
        "/obj/I_fireball_ncg.bin",
        "/obj/I_enemy_ncl.bin",
    },
    model = readModel("hammer_bros.nsbmd"),
}
