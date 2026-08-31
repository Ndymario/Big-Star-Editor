-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 198,
    name = "Balance lift",
    category = "Platforms and ropes",
    color = "#4f86c6",
    shape = "box",
    known = true,
    complete = true,
    notes = "Stops moving when it gets near to the top of the view.",
    grabBounds = { x = 0, y = 0, width = 112, height = 16 },
    renderBounds = { x = 0, y = 0, width = 112, height = 16 },
    sourceFiles = {
        "/enemy/b_lift.nsbtx (balance lift)",
    },
    fields = {
        {
            key = "loads_from",
            label = "Loads from",
            bitOffset = 20,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Level start" },
                { value = 1, label = "Midway point" },
            },
        },
        {
            key = "disappears",
            label = "Disappears",
            bitOffset = 12,
            bitWidth = 4,
            notes = "Disappears after a few seconds. 1 and 2 are unused.",
            choices = {
                { value = 0, label = "No" },
                { value = 1, label = "Yes" },
                { value = 2, label = "When Mario's off" },
            },
        },
        {
            key = "starts_w_o_player",
            label = "Starts w/o player",
            bitOffset = 4,
            bitWidth = 4,
            checkboxValue = 2,
        },
        {
            key = "directions_arrows",
            label = "Directions/Arrows",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Left Up Right" },
                { value = 1, label = "Left  Down Right" },
                { value = 2, label = "Left Up Right" },
                { value = 3, label = "Left Right" },
            },
        },
    },
    model = readModel("b_lift.nsbtx"),
}
