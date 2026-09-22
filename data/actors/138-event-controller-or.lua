-- Imported from NSMB Editor 5 stageobjsettings.xml.
local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16, height = 16 }
end


return {
    objectId = 138,
    name = "Event Controller OR",
    category = "Event controllers",
    notes = "Up to 4 inputs can trigger the same output ID: the Trigger event row is the first input, then Trigger ID 2-4, and the Target event row is the output. All input IDs must be set to relevant values. Do not leave any IDs as 0 - if you have fewer than four, you can enter the same ID into several spaces.",
    fields = {
        {
            key = "trigger_id_2",
            label = "Trigger ID 2",
            bitOffset = 24,
            bitWidth = 8,
        },
        {
            key = "trigger_id_3",
            label = "Trigger ID 3",
            bitOffset = 16,
            bitWidth = 8,
        },
        {
            key = "trigger_id_4",
            label = "Trigger ID 4",
            bitOffset = 8,
            bitWidth = 8,
        },
        {
            key = "mode",
            label = "Mode",
            bitOffset = 4,
            bitWidth = 4,
            notes = "How does 1 work?",
            choices = {
                { value = 0, label = "Single" },
                { value = 1, label = "Multi" },
                { value = 2, label = "While" },
            },
        },
        {
            key = "switch_mode",
            label = "Switch Mode",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Activates Target (Switches Event State)" },
                { value = 1, label = "Deactivates Target (Switches Event State)" },
            },
        },
    },
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
