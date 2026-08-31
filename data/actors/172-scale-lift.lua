-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 172,
    name = "Scale lift",
    category = "Platforms and ropes",
    color = "#4f86c6",
    shape = "box",
    known = true,
    complete = true,
    grabBounds = { x = -24, y = -10, width = 48, height = 18 },
    renderBounds = { x = -24, y = -10, width = 48, height = 18 },
    sourceFiles = {
        "/enemy/n_lift.nsbtx (normal_lift)",
        "/enemy/n_lift_line.nsbtx (normal_lift_line)",
        "/enemy/s_pulley.nsbmd (scale_pulley)",
    },
    fields = {
        {
            key = "left_side_rope_height",
            label = "Left side rope height",
            bitOffset = 8,
            bitWidth = 4,
        },
        {
            key = "right_side_rope_height",
            label = "Right side rope height",
            bitOffset = 4,
            bitWidth = 4,
        },
        {
            key = "distance_between_steps",
            label = "Distance between steps",
            bitOffset = 0,
            bitWidth = 4,
        },
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
    model = readModel("s_pulley.nsbmd"),
}
