-- Ported from NSMB Editor 5's NSMBStageObj.cs case 149.

local images = {}

local function init(ctx)
    images.Kabomb = ctx.loadImage("images/Kabomb.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    y = y - 3
    width = 16
    height = 19
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.Kabomb, RenderX, RenderY - 3) or drawn
    return drawn
end

return {
    objectId = 149,
    name = "Kab-omb",
    category = "Enemies - Fire",
    fields = {
        {
            key = "chases_mario",
            label = "Chases Mario",
            bitOffset = 0,
            bitWidth = 4,
            notes = "This makes him chase Mario if hit by a fireball/Raining Debris. If not set, he will run left and right. 0 (unchecked) is unused.",
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
