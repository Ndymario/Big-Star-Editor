-- Ported from NSMB Editor 5's NSMBStageObj.cs case 242.

local images = {}

local function init(ctx)
    images.PSwitch = ctx.loadImage("images/PSwitch.png")
    images.QSwitch = ctx.loadImage("images/QSwitch.png")
    images.RedSwitch = ctx.loadImage("images/RedSwitch.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local height = 16
    if not actor.fields.upside_down then
        y = y - 2
    end
    if actor.fields.shift_block_right then
        x = x + 8
    end
    height = 18
    return { x = x, y = y, width = 16, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local img
    local drawn = false
    img = images.QSwitch
    if actor.objectId == 244 then
        img = images.PSwitch
    end
    if actor.objectId == 246 then
        img = images.RedSwitch
    end
    if actor.fields.upside_down then
        img = gfx.transform(img, 180, false, false)
    else
        RenderY = RenderY - 2
    end
    if actor.fields.shift_block_right then
        RenderX = RenderX + 8
    end
    drawn = gfx.blit(renderer, img, RenderX, RenderY) or drawn
    return drawn
end

return {
    objectId = 242,
    name = "? Switch",
    category = "Event controllers",
    notes = "==Boss notes==\n* When sprite set 16 is set to any of the boss sets, the switch timer plays over the BG music, like in the Mega Goomba boss battle and with Red Rings. Though P Switches and ! Switches are silent with a boss sound set.\n* If the ? switch is absent in a Mega Goomba boss battle, the game freezes upon taking the key on a real NDS (not on DeSmuME though). Results are the same every time, if no ? switch is present, the game will freeze upon generation of the key (not even a brick block with ? switch will work). If there are more than 1 ? switch, the game will check for the leftmost switch first, then the bottommost if there are 2 or more switches on the same X coordinate, the one to fulfill this condition is the one to pop. [Source: Sharks]",
    fields = {
        {
            key = "triggering_event_id",
            label = "Triggering event ID",
            bitOffset = 40,
            bitWidth = 8,
            notes = "If not set, the switch will always show. If set, the switch will only show while this event ID is activated. Slightly buggy if used with multiple use as the switch will pop back up immediately, which looks a bit strange and also causes Mario to be pushed into the air. No \"real\" glitch.",
        },
        {
            key = "target_event_id",
            label = "Target event ID",
            bitOffset = 32,
            bitWidth = 8,
        },
        {
            key = "settings",
            label = "Settings",
            bitOffset = 28,
            bitWidth = 4,
            notes = "If 1, the switch will be movable by environment stuff, such as dropping down when bricks below it are destroyed, or riding along a conveyor belt. If 4, the switch will only show if the level has been started from a midway point. Value 2 is used in 2-3 A2. Value 3 is used in 3-Ghost House.",
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
            notes = "This will set how long the switch takes to pop up after the timer expires on a multiple use switch. Value 6 is used in World 3-Ghost House. Value 15 is used in 2-3 A2.",
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
            notes = "2 is used in 2-3 A2.",
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
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
