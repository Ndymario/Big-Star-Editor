-- Ported from NSMB Editor 5's NSMBStageObj.cs case 199.

local images = {}

local function init(ctx)
    images.GroundpoundGoo = ctx.loadImage("images/GroundpoundGoo.png")
end

local function grabBounds(actor)
    return {
        x = actor.pixelX,
        y = actor.pixelY,
        width = actor.fields.length * 16,
        height = actor.fields.height * 16,
    }
end

local function render(actor, renderer)
    return renderer:draw(images.GroundpoundGoo, grabBounds(actor))
end

return {
    objectId = 199,
    name = "Groundpound-able ghost house goo",
    category = "Platforms and ropes",
    fields = {
        {
            key = "length",
            label = "Length",
            bitOffset = 0,
            bitWidth = 4,
            valueOffset = 1,
        },
        {
            key = "height",
            label = "Height",
            bitOffset = 4,
            bitWidth = 4,
            valueOffset = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
