-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 42,
    name = "Balloon Boo",
    category = "Enemies - Ghost House",
    color = "#7b5aa6",
    shape = "ellipse",
    known = true,
    complete = false,
    notes = "As far as I know, only killable with a star, though I guess if it runs out of air it dies.",
    grabBounds = { x = -53, y = -106, width = 109, height = 108 },
    renderBounds = { x = -53, y = -106, width = 109, height = 108 },
    sourceFiles = {
        "/enemy/balloon_teresa.nsbmd",
    },
    fields = {
        {
            key = "stays_in_zone",
            label = "Stays in Zone",
            bitOffset = 16,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "zone_id",
            label = "Zone ID",
            bitOffset = 8,
            bitWidth = 8,
        },
        {
            key = "trigger_id_1",
            label = "Trigger ID 1",
            bitOffset = 32,
            bitWidth = 8,
            notes = "When waiting to be triggered, the area below Balloon Boo hurts Mario. You can't see anything there, but the game sees it as part of the sprite. When you activate the input ID, the area goes away, and Balloon Boo goes into a weird (unused or just glitchy?) state [in E version].",
        },
        {
            key = "trigger_id_2",
            label = "Trigger ID 2",
            bitOffset = 40,
            bitWidth = 8,
        },
        {
            key = "behaviour",
            label = "Behaviour",
            bitOffset = 4,
            bitWidth = 4,
            notes = "Unused, the Balloon Boo will act exactly like an original Boo, but the hitbox will be glitchy, so it is not generally recommended for use.",
            choices = {
                { value = 0, label = "Default" },
                { value = 1, label = "Acts like a Boo" },
            },
        },
    },
    model = readModel("balloon_teresa.nsbmd"),
}
