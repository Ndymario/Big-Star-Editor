-- Ported from NSMB Editor 5's NSMBStageObj.cs case 187.

local images = {}

local function init(ctx)
    images.StarGate = ctx.loadImage("images/StarGate.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    width = 32
    height = 8
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.StarGate, RenderX, RenderY) or drawn
    return drawn
end

return {
    objectId = 187,
    name = "Ground-pound panel",
    category = "Platforms and ropes",
    notes = "Needs a ground-pound to be able to pass from top. Is jump-through from bottom.",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
