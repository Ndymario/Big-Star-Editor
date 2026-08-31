-- Ported from NSMB Editor 5's NSMBStageObj.cs case 246.

local images = {}

local function init(ctx)
    images.RedSwitch = ctx.loadImage("images/RedSwitch.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    if not actor.fields.upside_down then
        y = y - 2
    end
    if actor.fields.shift_block_right then
        x = x + 8
    end
    return { x = x, y = y, width = 16, height = 18 }
end

local function render(actor, renderer)
    local box = grabBounds(actor)
    if actor.fields.upside_down then
        box.rotation = 180
    end
    return renderer:draw(images.RedSwitch, box)
end

return {
    objectId = 246,
    name = "! Switch",
    category = "Event controllers",
    fields = {
        {
            key = "triggering_event_id",
            label = "Triggering event ID",
            bitOffset = 40,
            bitWidth = 8,
        },
        {
            key = "target_event_id",
            label = "Target event ID",
            bitOffset = 32,
            bitWidth = 8,
            notes = "By default, this switch will turn yellow outlines into uncollectible golden coins and red outlines into red coins which act like golden coins; snake block tiles will be collectible as coins and the block-shaped outlines will turn into red blocks.",
        },
        {
            key = "settings",
            label = "Settings",
            bitOffset = 28,
            bitWidth = 4,
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
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
