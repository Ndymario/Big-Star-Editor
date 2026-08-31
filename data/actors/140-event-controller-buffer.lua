-- Imported from NSMB Editor 5 stageobjsettings.xml.
local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16, height = 16 }
end


return {
    objectId = 140,
    name = "Event Controller Buffer",
    category = "Event controllers",
    notes = "Activates multiple events. It's possible to define a delay time. If the delay is set to 0, the event will be activated instantly. If not, it will wait a specific amount of time before activating.\n\nThe purpose of this sprite in World 3-Ghost House seems to be to make the stairs go down a few seconds after the ? Switch timer is up.",
    fields = {
        {
            key = "trigger_id",
            label = "Trigger ID",
            bitOffset = 40,
            bitWidth = 8,
            notes = "When this event ID is activated, the target event ID will be activated or deactivated (depending on the mode).",
        },
        {
            key = "target_id_1",
            label = "Target ID 1",
            bitOffset = 32,
            bitWidth = 8,
            notes = "When activated, this event ID will be switched (activated or deactivated), depending on the chosen mode. Note: Deactivating a deactivated event will activate it. Activating an already activated event will deactivate it.",
        },
        {
            key = "trigger_mode",
            label = "Trigger mode",
            bitOffset = 20,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Inactive" },
                { value = 1, label = "Triggered" },
            },
        },
        {
            key = "timeout_mode",
            label = "Timeout mode",
            bitOffset = 16,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Activate" },
                { value = 1, label = "Deactivate" },
            },
        },
        {
            key = "activation_delay",
            label = "Activation delay",
            bitOffset = 24,
            bitWidth = 8,
            notes = "If the delay is set to 0, the event will be activated instantly. If not, it will wait a specific amount of time before activating. Info: A Value of 6 is approximately 1 tick (1 second on the game's clock).",
        },
        {
            key = "disable_sprite",
            label = "Disable sprite",
            bitOffset = 16,
            bitWidth = 4,
            notes = "Does nothing/self destructs.",
            checkboxValue = 1,
        },
        {
            key = "mode",
            label = "Mode",
            bitOffset = 4,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Single" },
                { value = 1, label = "Multi" },
                { value = 2, label = "While" },
            },
        },
        {
            key = "switch_target_mode",
            label = "Switch target mode",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Activates target" },
                { value = 1, label = "Deactivates target" },
            },
        },
    },
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
