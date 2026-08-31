-- Ported from NSMB Editor 5's NSMBStageObj.cs case 188.

local images = {}

local function init(ctx)
    images.BumpPlatformLeft = ctx.loadImage("images/BumpPlatformLeft.png")
    images.BumpPlatformRight = ctx.loadImage("images/BumpPlatformRight.png")
    images.BumpPlatformSection = ctx.loadImage("images/BumpPlatformSection.png")
end

local function grabBounds(actor)
    local length = actor.fields.length
    return {
        x = actor.pixelX - (length * 8 + 6),
        y = actor.pixelY,
        width = 16 * (length - 1) + 28,
        height = 10,
    }
end

local function render(actor, renderer)
    local box = grabBounds(actor)
    local length = actor.fields.length
    renderer:draw(images.BumpPlatformLeft, { x = box.x, y = box.y })
    for section = 0, length - 2 do
        renderer:draw(images.BumpPlatformSection, { x = box.x + 14 + 16 * section, y = box.y })
    end
    if length > 0 then
        return renderer:draw(images.BumpPlatformRight, {
            x = box.x + 14 + 16 * (length - 1),
            y = box.y,
        })
    end
    return true
end

return {
    objectId = 188,
    name = "Bump from below platform (Mario Bros. style platform)",
    category = "Platforms and ropes",
    fields = {
        {
            key = "length",
            label = "Length",
            bitOffset = 0,
            bitWidth = 8,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
