-- Imported from NSMB Editor 5 stageobjsettings.xml.
local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16, height = 16 }
end


return {
    objectId = 137,
    name = "Event Controller AND",
    category = "Event controllers",
    notes = "This sprite works by allowing up to 4 input IDs to be required to activate a single output. All input IDs must be set to a relevant value. Do not leave any as '0' - if you have less than four, you can enter the same ID into several spaces.",
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
            notes = "Determines, if and how often the relevant Event IDs will be triggered. Events with Values '0' (Single Use) or '1' (Multi Use) will keep their status, unless deactivated/reactivated manually.",
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
