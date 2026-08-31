-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 24,
    name = "Pipe Piranha Plant (facing down)",
    category = "Enemies - Overground",
    color = "#b5651d",
    shape = "ellipse",
    known = true,
    complete = true,
    grabBounds = { x = 0, y = 32, width = 32, height = 28 },
    renderBounds = { x = 0, y = 32, width = 32, height = 28 },
    sourceFiles = {
        "/enemy/v_pakkun.nsbmd",
    },
    fields = {
        {
            key = "number_of_fireballs",
            label = "Number of fireballs",
            bitOffset = 0,
            bitWidth = 8,
            notes = "1+ is unused, but some are used in NSMBWii.",
            choices = {
                { value = 0, label = "0" },
                { value = 1, label = "1" },
                { value = 17, label = "2" },
                { value = 33, label = "3" },
                { value = 49, label = "6" },
            },
        },
    },
    model = readModel("v_pakkun.nsbmd"),
}
