-- Ported from NSMB Editor 5's NSMBStageObj.cs case 120.

local images = {}

local function init(ctx)
    images.Broozer = ctx.loadImage("images/Broozer.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY - 15, width = 45, height = 31 }
end

local function render(actor, renderer)
    return renderer:draw(images.Broozer, grabBounds(actor))
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
