-- Ported from NSMB Editor 5's NSMBStageObj.cs case 106.

local images = {}

local function init(ctx)
    images.JumpingFlame = ctx.loadImage("images/JumpingFlame.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    y = y - 7
    width = 27
    height = 23
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.JumpingFlame, RenderX, RenderY - 7) or drawn
    return drawn
end

return {
    objectId = 106,
    name = "Fire Snake",
    category = "Enemies - Desert",
    notes = "A snake made of fire that follows you by jumping. It's a very annoying enemy!",
    fields = {
        {
            key = "jump",
            label = "jump",
            bitOffset = 40,
            bitWidth = 4,
            notes = "do not use on solid walls, game error",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
