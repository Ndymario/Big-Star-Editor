-- Ported from NSMB Editor 5's NSMBStageObj.cs case 120.

local images = {}

local function init(ctx)
    images.Broozer = ctx.loadImage("images/Broozer.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    y = y - 15
    width = 45
    height = 31
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.Broozer, RenderX, RenderY - 15) or drawn
    return drawn
end

return {
    objectId = 120,
    name = "Broozer",
    category = "Enemies - Ghost House",
    notes = "The punching ghost. It can break breakable blocks. It's sound effects are in the ghost house music. Uses the tile god actor to destroy blocks, for some reason. < Is that last bit actually true? I didn't see any tile god listed when using the sprite info printer.[Reply] So, delete it!",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
