-- Ported from NSMB Editor 5's NSMBStageObj.cs case 254.

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
    objectId = 254,
    name = "Spin Block",
    category = "Platforms and ropes",
    notes = "Random notes:\n- When the player is fully submerged in water, they can only use the spin block when ducking, and even then, it just propels the swimming player upward - not putting them into \"spinning mode\". When the player reaches the surface (if there is one), they stop going up.",
    fields = {
        {
            key = "trigger_id",
            label = "Trigger ID",
            bitOffset = 32,
            bitWidth = 8,
            notes = "Swiches and controllers",
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
        {
            key = "spin_jump",
            label = "Spin Jump",
            bitOffset = 0,
            bitWidth = 4,
            notes = "does not need to have a value to appear",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
