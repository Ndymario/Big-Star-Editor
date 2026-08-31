-- Ported from NSMB Editor 5's NSMBStageObj.cs case 256.

local images = {}

local function init(ctx)
    images.SpinBoard = ctx.loadImage("images/SpinBoard.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 6
    width = 26
    height = 26
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.SpinBoard, RenderX - 6, RenderY) or drawn
    return drawn
end

return {
    objectId = 256,
    name = "Spin Block (unused Actor 256)",
    category = "Platforms and ropes",
    notes = "Not complete because: Unknown values, unknown use\nWon't appear if Sprite 95 is not in immediate vicinity.",
    fields = {
        {
            key = "trigger_id",
            label = "Trigger ID",
            bitOffset = 32,
            bitWidth = 8,
        },
        {
            key = "unknown_8",
            label = "Unknown 8",
            bitOffset = 12,
            bitWidth = 4,
            notes = "Used in-game. Doesn't do anything notable.",
        },
        {
            key = "unknown_9",
            label = "Unknown 9",
            bitOffset = 8,
            bitWidth = 4,
            notes = "Used in-game. Doesn't do anything notable.",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
