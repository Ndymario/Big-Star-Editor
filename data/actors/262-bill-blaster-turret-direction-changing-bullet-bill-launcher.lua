-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 262,
    name = "Bill Blaster Turret (Direction changing Bullet Bill launcher)",
    category = "Enemies - Castle",
    color = "#535967",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "First checkboxes are bottom cannons.",
    grabBounds = { x = -3, y = 0, width = 23, height = 16 },
    renderBounds = { x = -3, y = 0, width = 23, height = 16 },
    sourceFiles = {
        "/enemy/killer.nsmbd",
    },
    fields = {
        {
            key = "empty_spots",
            label = "Empty spots",
            bitOffset = 8,
            bitWidth = 8,
            notes = "individual bits indicate empty spots",
        },
        {
            key = "cannons_to_flip",
            label = "Cannons to flip",
            bitOffset = 0,
            bitWidth = 8,
            notes = "individual bits indicate flipped cannons",
        },
        {
            key = "height",
            label = "Height",
            bitOffset = 16,
            bitWidth = 4,
            notes = "Height is 1 + (N7 mod 8)",
            valueOffset = 1,
        },
        {
            key = "delayed_firing",
            label = "Delayed firing",
            bitOffset = 28,
            bitWidth = 4,
            notes = "Unused.",
            checkboxValue = 1,
        },
    },
}
