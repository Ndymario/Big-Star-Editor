-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 173,
    name = "Flimsy Lift (falls once touched)",
    category = "Platforms and ropes",
    color = "#4f86c6",
    shape = "box",
    known = true,
    complete = false,
    grabBounds = { x = -8, y = 0, width = 32, height = 16 },
    renderBounds = { x = -8, y = 0, width = 32, height = 16 },
    sourceFiles = {
        "/enemy/n_lift.nsbtx",
    },
    fields = {
        {
            key = "length",
            label = "Length",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Length formula is 2+2×(N11 mod 4). Everything other than 1 is unused. Certain lengths seem to cause it to shift upwards.",
            valueOffset = 1,
        },
        {
            key = "length_2",
            label = "length",
            bitOffset = 40,
            bitWidth = 4,
            notes = "do not exceed 4 or it will not be displayed",
        },
    },
    model = readModel("n_lift.nsbtx"),
}
