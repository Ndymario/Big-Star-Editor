-- Ported from NSMB Editor 5's NSMBStageObj.cs case 201.

local images = {}

local function init(ctx)
    images.CloudLeftEdge = ctx.loadImage("images/CloudLeftEdge.png")
    images.CloudRightEdge = ctx.loadImage("images/CloudRightEdge.png")
    images.CloudSection = ctx.loadImage("images/CloudSection.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    height = 32
    width = 32 * (d[5] + 2)
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.CloudLeftEdge, RenderX, RenderY) or drawn
    RenderX = RenderX + 16
    for l = 0, d[5] do
        drawn = gfx.blit(renderer, images.CloudSection, RenderX, RenderY) or drawn
        RenderX = RenderX + 32
    end
    drawn = gfx.blit(renderer, images.CloudRightEdge, RenderX, RenderY) or drawn
    return drawn
end

return {
    objectId = 201,
    name = "Squishy cloud platform",
    category = "Platforms and ropes",
    notes = "This squishes under Mario's feet.\n\nThere is an unused (except off screen) tile version in the tileset \"Sky (World 7)\". Apparently has a unseen graphic where all of it  is squished. Only viewable with a Mega Mushroom, though.",
    fields = {
        {
            key = "length",
            label = "Length",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Cloud extends from sprite position to the right.",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
