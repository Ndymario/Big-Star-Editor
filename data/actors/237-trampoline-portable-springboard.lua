-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 237,
    name = "Trampoline (portable springboard)",
    category = "Platforms and ropes",
    color = "#4f86c6",
    shape = "box",
    known = true,
    complete = true,
    notes = "An item used to make Mario jump higher. Can be carried by Mario. Objects can push it (but only in later versions?).\n\nBounces the player up by 6 tiles (11 if mini).\n\nThe big version bounces the player up by 18 tiles (29 if mini).\n\nThe pickup hitbox for the giant springboard is far too large and extends even above it, which makes you pick it up if you're holding Y and don't press B in time when springing off it. (hitbox information & text: skawo)",
    grabBounds = { x = 0, y = 0, width = 16, height = 16 },
    renderBounds = { x = 0, y = 0, width = 16, height = 16 },
    sourceFiles = {
        "/enemy/jump_step.nsbmd",
    },
    fields = {
        {
            key = "shift_block_right",
            label = "Shift ½ block right",
            bitOffset = 4,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "larger",
            label = "Larger",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Unused. (Can get stuck in walls)",
            checkboxValue = 1,
        },
    },
    model = readModel("jump_step.nsbmd"),
}
