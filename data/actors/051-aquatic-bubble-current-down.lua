-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 51,
    name = "Aquatic bubble current (down)",
    category = "Enemies - Water",
    color = "#3f7fc4",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "Setting both input IDs to zero will make the bubbles always run.",
    grabBounds = { x = 0, y = 32, width = 32, height = 74 },
    renderBounds = { x = 0, y = 32, width = 32, height = 74 },
    sourceFiles = {
        "/obj/l_awa_ncg.bin",
    },
    fields = {
        {
            key = "trigger_id_1",
            label = "Trigger ID 1",
            bitOffset = 32,
            bitWidth = 8,
        },
        {
            key = "trigger_id_2",
            label = "Trigger ID 2",
            bitOffset = 40,
            bitWidth = 8,
        },
    },
}
