-- Imported from NSMB Editor 5 stageobjsettings.xml.
local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16, height = 16 }
end


return {
    objectId = 245,
    name = "Brick Block containing P Switch",
    category = "Event controllers",
    notes = "Target event: if not set, the switch will always show. If set, the switch will only show while this event ID is activated. Slightly buggy if used with multiple use, as the switch will pop back up immediately, which looks a bit strange and also causes Mario to be pushed into the air. No \"real\" glitch.",
    fields = {
        {
            key = "settings",
            label = "Settings",
            bitOffset = 28,
            bitWidth = 4,
            notes = "If 1, the switch will be movable by environment stuff, such as dropping down when bricks below it are destroyed, or riding along a conveyor belt. If 4, the switch will only show if the level has been started from a midway point.",
            choices = {
                { value = 0, label = "None" },
                { value = 1, label = "Falls" },
                { value = 2, label = "Unknown 2" },
                { value = 3, label = "Unknown 3" },
                { value = 4, label = "Loads at midway" },
            },
        },
        {
            key = "delay_on_multiple_use",
            label = "Delay on multiple use",
            bitOffset = 24,
            bitWidth = 4,
            notes = "This will set how long the switch takes to pop up after the timer expires on a multiple use switch.",
            choices = {
                { value = 0, label = "Instantaneous (no delay)" },
                { value = 2, label = "Short delay" },
                { value = 3, label = "Long delay" },
                { value = 1, label = "Very long delay" },
                { value = 15, label = "Unknown" },
                { value = 6, label = "Unknown 6" },
            },
        },
        {
            key = "non_timed_event",
            label = "Non-timed event",
            bitOffset = 20,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "switch_mode",
            label = "Switch mode",
            bitOffset = 12,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Activates target (switches event state)" },
                { value = 1, label = "Deactivates target (switches event state)" },
            },
        },
        {
            key = "usage_mode",
            label = "Usage mode",
            bitOffset = 8,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Single use" },
                { value = 1, label = "Multiple use" },
                { value = 2, label = "Unknown" },
            },
        },
        {
            key = "shift_block_right",
            label = "Shift ½ block right",
            bitOffset = 4,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "upside_down",
            label = "Upside down",
            bitOffset = 0,
            bitWidth = 4,
            notes = "If this is checked, the switch will be facing down. Upside down switches cannot fall.",
            checkboxValue = 1,
        },
    },
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
