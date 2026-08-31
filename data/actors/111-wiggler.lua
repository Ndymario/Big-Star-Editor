-- Ported from NSMB Editor 5's NSMBStageObj.cs case 111.

local images = {}

local function init(ctx)
    images.Wiggler = ctx.loadImage("images/Wiggler.png")
    images.WigglerL = ctx.loadImage("images/WigglerL.png")
end

local function grabBounds(actor)
    if actor.fields.bigger then
        return { x = actor.pixelX, y = actor.pixelY - 44, width = 92, height = 60 }
    end
    return { x = actor.pixelX, y = actor.pixelY - 14, width = 52, height = 30 }
end

local function render(actor, renderer)
    if actor.fields.bigger and actor.fields.unknown_10 == 0 then
        return renderer:draw(images.WigglerL, {
            x = actor.pixelX, y = actor.pixelY - 44, width = 92, height = 60,
        })
    end
    return renderer:draw(images.Wiggler, {
        x = actor.pixelX, y = actor.pixelY - 14, width = 52, height = 30,
    })
end

return {
    objectId = 111,
    name = "Wiggler",
    category = "Enemies - Overground",
    fields = {
        {
            key = "bigger",
            label = "Bigger",
            bitOffset = 0,
            bitWidth = 4,
            notes = "1 is unused and gives it glitchy animations and hitbox.",
            checkboxValue = 1,
        },
        {
            key = "unknown_10",
            label = "Unknown 10",
            bitOffset = 4,
            bitWidth = 4,
        },
        {
            key = "unknown_4",
            label = "Unknown 4",
            bitOffset = 28,
            bitWidth = 4,
            notes = "Used in W4-4",
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
