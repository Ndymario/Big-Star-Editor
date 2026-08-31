-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 171,
    name = "Seesaw lift",
    category = "Platforms and ropes",
    color = "#4f86c6",
    shape = "box",
    known = true,
    complete = true,
    notes = "A.K.A\n* See-saw Platform",
    grabBounds = { x = 0, y = 0, width = 256, height = 16 },
    renderBounds = { x = 0, y = 0, width = 256, height = 16 },
    sourceFiles = {
        "/enemy/seesaw.nsbmd",
    },
    fields = {
        {
            key = "0_disable_1_enable_trigger_id_check",
            label = "0=Disable/1=Enable Trigger ID check",
            bitOffset = 28,
            bitWidth = 4,
            checkboxValue = 1,
        },
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
    model = readModel("seesaw.nsbmd"),
}
