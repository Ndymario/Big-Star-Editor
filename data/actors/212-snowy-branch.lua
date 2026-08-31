-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 212,
    name = "Snowy branch",
    category = "Enemies - Snow",
    color = "#65a9c7",
    shape = "ellipse",
    known = true,
    complete = true,
    grabBounds = { x = 15, y = -12, width = 30, height = 23 },
    renderBounds = { x = 15, y = -12, width = 30, height = 23 },
    sourceFiles = {
        "/enemy/I_mominoki_eda.nsbmd",
        "/enemy/I_mominoki_eda2.nsbmd",
        "/enemy/I_mominoki_yuki2.nsbmd",
        "/enemy/I_mominoki_yuki3.nsbmd",
    },
    fields = {
        {
            key = "branch_side",
            label = "Branch side",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Right" },
                { value = 1, label = "Left" },
            },
        },
    },
    model = readModel("I_mominoki_eda.nsbmd"),
}
