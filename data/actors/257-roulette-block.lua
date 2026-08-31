-- Ported from NSMB Editor 5's NSMBStageObj.cs case 257.

local images = {}

local function init(ctx)
    images.RouletteBlock = ctx.loadImage("images/RouletteBlock.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 20, height = 20 }
end

local function render(actor, renderer)
    return renderer:draw(images.RouletteBlock, grabBounds(actor))
end

return {
    objectId = 257,
    name = "Roulette Block",
    category = "Other",
    fields = {
        {
            key = "extra_item",
            label = "Extra item",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Higher than 1 is unused.",
            choices = {
                { value = 0, label = "Blue Shell" },
                { value = 1, label = "Mini Mushroom" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
