-- Ported from NSMB Editor 5's NSMBStageObj.cs case 201.

local images = {}

local function init(ctx)
    images.CloudLeftEdge = ctx.loadImage("images/CloudLeftEdge.png")
    images.CloudRightEdge = ctx.loadImage("images/CloudRightEdge.png")
    images.CloudSection = ctx.loadImage("images/CloudSection.png")
end

local function grabBounds(actor)
    return {
        x = actor.pixelX,
        y = actor.pixelY,
        width = 32 * (actor.fields.length + 2),
        height = 32,
    }
end

local function render(actor, renderer)
    renderer:draw(images.CloudLeftEdge, { x = actor.pixelX, y = actor.pixelY })
    for section = 0, actor.fields.length do
        renderer:draw(images.CloudSection, {
            x = actor.pixelX + 16 + 32 * section,
            y = actor.pixelY,
        })
    end
    return renderer:draw(images.CloudRightEdge, {
        x = actor.pixelX + 16 + 32 * (actor.fields.length + 1),
        y = actor.pixelY,
    })
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
            bitWidth = 8,
            notes = "Cloud extends from sprite position to the right.",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
