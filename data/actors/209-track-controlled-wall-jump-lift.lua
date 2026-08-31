-- Ported from NSMB Editor 5's NSMBStageObj.cs case 209.

local images = {}

local function init(ctx)
    images.LineAttachment = ctx.loadImage("images/LineAttachment.png")
    images.WallJumpPlatformBlock = ctx.loadImage("images/WallJumpPlatformBlock.png")
end

local function grabBounds(actor)
    return {
        x = actor.pixelX - 8 * (actor.fields.length - 4) - 24,
        y = actor.pixelY + 5,
        width = 16 * actor.fields.length,
        height = 16 * actor.fields.height + 19,
    }
end

local function render(actor, renderer)
    local box = grabBounds(actor)
    local rightX = box.x + 16 * (actor.fields.length - 1)
    for row = actor.fields.height, 1, -1 do
        local y = actor.pixelY + 8 + 16 * row
        renderer:draw(images.WallJumpPlatformBlock, { x = box.x, y = y })
        renderer:draw(images.WallJumpPlatformBlock, { x = rightX, y = y })
    end
    for column = 1, actor.fields.length - 2 do
        renderer:draw(images.WallJumpPlatformBlock, {
            x = box.x + 16 * column,
            y = actor.pixelY + 24,
        })
    end
    return renderer:draw(images.LineAttachment, { x = actor.pixelX - 3, y = actor.pixelY + 5 })
end

return {
    objectId = 209,
    name = "Track-controlled wall-jump lift",
    category = "Platforms and ropes",
    fields = {
        {
            key = "starting_direction",
            label = "Starting direction",
            bitOffset = 16,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Right" },
                { value = 1, label = "Left" },
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
                { value = 1, label = "When touched at least once" },
            },
        },
        {
            key = "length",
            label = "Length",
            bitOffset = 0,
            bitWidth = 2,
            notes = "Width is 4+(N11 mod 4)",
            valueOffset = 4,
        },
        {
            key = "height",
            label = "Height",
            bitOffset = 12,
            bitWidth = 2,
            notes = "Height is 4+(N8 mod 4)",
            valueOffset = 4,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
