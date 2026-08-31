-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 126,
    name = "Squiggler",
    category = "Enemies - Underground",
    color = "#686d76",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "A caterpillar that is smaller than the Wiggler. Crawls slowly on all sides of  a surface.",
    grabBounds = { x = 10, y = -15, width = 13, height = 15 },
    renderBounds = { x = 10, y = -15, width = 13, height = 15 },
    sourceFiles = {
        "/enemy/I_wakaba.nsbmd",
        "/enemy/wakaba.nsbmd",
        "/enemy/wakaba_yoko.nsbmd",
        "/sound_data.sdat/SeqArc/SAR_ENEMY_02_SE_EMY_WAKABA_APP",
    },
    fields = {
        {
            key = "pipe_exit_facing",
            label = "Pipe exit facing",
            bitOffset = 0,
            bitWidth = 4,
            notes = "2 and 3 are unused.",
            choices = {
                { value = 0, label = "Up" },
                { value = 1, label = "Down" },
                { value = 3, label = "Left" },
                { value = 2, label = "Right" },
            },
        },
    },
    model = readModel("I_wakaba.nsbmd"),
}
