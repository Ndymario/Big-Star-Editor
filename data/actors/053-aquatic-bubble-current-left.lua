-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 53,
    name = "Aquatic bubble current (left)",
    category = "Enemies - Water",
    color = "#3f7fc4",
    shape = "ellipse",
    known = true,
    complete = true,
    grabBounds = { x = -90, y = 0, width = 74, height = 32 },
    renderBounds = { x = -90, y = 0, width = 74, height = 32 },
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
