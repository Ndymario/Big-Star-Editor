-- Ported from NSMB Editor 5's NSMBStageObj.cs case 244.

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
    objectId = 244,
    name = "P Switch",
    category = "Event controllers",
    notes = "This automatically makes all Blue Coins appear.",
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
            notes = "By default, this switch will activate blue coins and turn coins into brick blocks and vice versa. Don't use this switch in combination with a tile god which creates blue coins; said blue coins' graphics will be glitched, so use blue coins objects instead.",
        },
        {
            key = "settings",
            label = "Settings",
            bitOffset = 28,
            bitWidth = 4,
            notes = "If 1, the switch will be movable by environment stuff, such as dropping down when bricks below it are destroyed, or riding along a conveyor belt. If 4, the switch will only show if the level has been started from a midway point.",
            choices = {
                { value = 0, label = "None" },
                { value = 1, label = "Falls" },
                { value = 4, label = "Loads at midway" },
            },
        },
        {
            key = "delay_on_multiple_use",
            label = "Delay on multiple use",
            bitOffset = 24,
            bitWidth = 4,
            notes = "This will set how long the switch takes to pop up after the timer expires on a multiple-use switch.",
            choices = {
                { value = 0, label = "Instantaneous (no delay)" },
                { value = 2, label = "Short delay" },
                { value = 3, label = "Long delay" },
                { value = 1, label = "Very long delay" },
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
                { value = 2, label = "Unknown. 2 is unused." },
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
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
