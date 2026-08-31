-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 36,
    name = "Goal Pole's flag",
    category = "Bosses and cutscenes",
    color = "#b84b4b",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "AKA Flagpole Flag",
    grabBounds = { x = -29, y = -119, width = 27, height = 24 },
    renderBounds = { x = -29, y = -119, width = 27, height = 24 },
    sourceFiles = {
        "/enemy/all_goal_flag.nsbmd [may be unused]",
        "/enemy/all_goal_flagR.nsbmd (all_goal_flagRed) [may be unused]",
        "/enemy/obj_goal_flag.nsbmd (object_goal_flag)[may be unused]",
        "/enemy/obj_goal_flagR.nsbmd (object_goal_flagRed) [may be unused]",
    },
    fields = {
        {
            key = "secret_exit",
            label = "Secret exit",
            bitOffset = 28,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "upright_with_pipe_cannon",
            label = "Upright with Pipe Cannon",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Mario will grab the flag upright initially, instead of upside down, when fired from a Pipe Cannon. 1 seems to be the same as 0.",
            checkboxValue = 2,
        },
    },
    model = readModel("all_goal_flag.nsbmd"),
}
