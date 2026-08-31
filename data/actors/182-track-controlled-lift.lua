-- Ported from NSMB Editor 5's NSMBStageObj.cs case 182.

local images = {}

local function init(ctx)
    images.MovingPlatformLeft = ctx.loadImage("images/MovingPlatformLeft.png")
    images.MovingPlatformRight = ctx.loadImage("images/MovingPlatformRight.png")
    images.MovingPlatformSection = ctx.loadImage("images/MovingPlatformSection.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY + 8, width = 64, height = 16 }
end

local function render(actor, renderer)
    local box = grabBounds(actor)
    renderer:draw(images.MovingPlatformLeft, { x = box.x, y = box.y })
    renderer:draw(images.MovingPlatformSection, { x = box.x + 16, y = box.y })
    renderer:draw(images.MovingPlatformSection, { x = box.x + 32, y = box.y })
    return renderer:draw(images.MovingPlatformRight, { x = box.x + 48, y = box.y })
end

return {
    objectId = 182,
    name = "Track-controlled lift",
    category = "Platforms and ropes",
    notes = "You can't make it longer, so try putting two beside each other without \"Wait for Mario\" checked.",
    fields = {
        {
            key = "starting_direction",
            label = "Starting direction",
            bitOffset = 16,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Up/Right" },
                { value = 1, label = "Down/Left" },
            },
        },
        {
            key = "speed",
            label = "Speed",
            bitOffset = 24,
            bitWidth = 4,
            choices = {
                { value = 0, label = "60 pixels/sec" },
                { value = 1, label = "90 pixels/sec" },
                { value = 2, label = "120 pixels/sec" },
                { value = 3, label = "180 pixels/sec" },
            },
        },
        {
            key = "movement_trigger",
            label = "Movement trigger",
            bitOffset = 20,
            bitWidth = 4,
            choices = {
                { value = 0, label = "On its own" },
                { value = 1, label = "Once stood on" },
            },
        },
        {
            key = "enable_trigger_id_check",
            label = "Enable trigger ID check",
            bitOffset = 28,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "trigger_id_1",
            label = "Trigger ID 1",
            bitOffset = 32,
            bitWidth = 8,
        },
        {
            key = "trigger_id_2",
            label = "Trigger ID 2",
            bitOffset = 40,
            bitWidth = 8,
        },
        {
            key = "unknown_11",
            label = "Unknown 11",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Used as 2 and 6 in Unused Level 1.",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
