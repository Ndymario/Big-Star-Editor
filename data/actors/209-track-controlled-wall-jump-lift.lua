-- Ported from NSMB Editor 5's NSMBStageObj.cs case 209.

local images = {}

local function init(ctx)
    images.LineAttachment = ctx.loadImage("images/LineAttachment.png")
    images.WallJumpPlatformBlock = ctx.loadImage("images/WallJumpPlatformBlock.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - (d[5] % 0x4 * 8 + 24)
    y = y + 5
    width = d[5] % 0x4 * 16 + 64
    height = (d[4] & 0x30) // 0x10 * 16 + 83
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    RenderX = RenderX - (d[5] % 0x4 * 8 + 24)
    RenderY = RenderY + ((d[4] & 0x30) // 0x10 * 16 + 88)
    local XOff = d[5] % 0x4 * 16 + 48
    for l = 0, ((d[4] & 0x30) // 0x10 + 4) - 1 do
        RenderY = RenderY - 16
        drawn = gfx.blit(renderer, images.WallJumpPlatformBlock, RenderX, RenderY) or drawn
        drawn = gfx.blit(renderer, images.WallJumpPlatformBlock, RenderX + XOff, RenderY) or drawn
    end
    for l = 0, (d[5] % 0x4 + 2) - 1 do
        RenderX = RenderX + 16
        drawn = gfx.blit(renderer, images.WallJumpPlatformBlock, RenderX, RenderY) or drawn
    end
    drawn = gfx.blit(renderer, images.LineAttachment, (actor.pixelX / 16) * 16 - 3, (actor.pixelY / 16) * 16 + 5) or drawn
    return drawn
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
            bitWidth = 4,
            notes = "Width is 4+(N11 mod 4)",
            valueOffset = 4,
        },
        {
            key = "height",
            label = "Height",
            bitOffset = 12,
            bitWidth = 4,
            notes = "Height is 4+(N8 mod 4)",
            valueOffset = 4,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
