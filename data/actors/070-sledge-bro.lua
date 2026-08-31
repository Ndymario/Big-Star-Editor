-- Ported from NSMB Editor 5's NSMBStageObj.cs case 70.

local images = {}

local function init(ctx)
    images.GiantHammerBro = ctx.loadImage("images/GiantHammerBro.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY - 15, width = 26, height = 31 }
end

local function render(actor, renderer)
    return renderer:draw(images.GiantHammerBro, grabBounds(actor))
end

return {
    objectId = 70,
    name = "Sledge Bro.",
    category = "Enemies - Overground",
    notes = "Can destroy smashable (question mark/brick/wood/stone/used) blocks.",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
