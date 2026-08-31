-- Imported from NSMB Editor 5 stageobjsettings.xml.
local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16, height = 16 }
end


return {
    objectId = 243,
    name = "Brick Block containing ? Switch",
    category = "Event controllers",
    notes = "==Boss notes==\n* When sprite set 16 is set to any of the boss sets, the switch timer plays over the BG music, like in the Mega Goomba boss battle and with Red Rings. Though P Switches and ! Switches are silent with a boss sound set.\n* If the ? switch is absent in a Mega Goomba boss battle, the game freezes upon taking the key on a real NDS (not on DeSmuME though). Results are the same every time, if no ? switch is present, the game will freeze upon generation of the key (not even a brick block with ? switch will work). If there are more than 1 ? switch, the game will check for the leftmost switch first, then the bottommost if there are 2 or more switches on the same X coordinate, the one to fulfill this condition is the one to pop. [Source: Sharks]",
    fields = {
        {
            key = "trigger_id",
            label = "Trigger ID",
            bitOffset = 40,
            bitWidth = 8,
            notes = "If not set, the switch will always show. If set, the switch will only show while this event ID is activated. Slightly buggy if used with multiple use as the switch will pop back up immediately, which looks a bit strange and also causes Mario to be pushed into the air. No \"real\" glitch.",
        },
        {
            key = "target_id",
            label = "Target ID",
            bitOffset = 32,
            bitWidth = 8,
        },
        {
            key = "settings",
            label = "Settings",
            bitOffset = 28,
            bitWidth = 4,
            notes = "If 1, the switch will be movable by environment stuff, such as dropping down when bricks below it are destroyed, or riding along a conveyor belt. If 4, the switch will only show if the level has been started from a midway point.  Value 2 is used in 2-3, it may have something to do with the target ID.",
            choices = {
                { value = 0, label = "None" },
                { value = 1, label = "Falls" },
                { value = 4, label = "Loads at midway" },
                { value = 2, label = "Unknown [see note]" },
            },
        },
        {
            key = "delay_on_multiple_use",
            label = "Delay on multiple use",
            bitOffset = 24,
            bitWidth = 4,
            notes = "[Check this nybble]. This will set how long the switch takes to pop up after the timer expires on a multiple use switch. Value 15 may have something to do with the timer. Maybe it pops up just after target ID is deactivated after activation with timer?",
            choices = {
                { value = 0, label = "Instantaneous (no delay)" },
                { value = 2, label = "Short delay" },
                { value = 3, label = "Long delay" },
                { value = 1, label = "Very long delay" },
                { value = 15, label = "Unknown" },
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
