-- Ported from NSMB Editor 5's NSMBStageObj.cs case 257.

local images = {}

local function init(ctx)
    images.RouletteBlock = ctx.loadImage("images/RouletteBlock.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    width = 20
    height = 20
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.RouletteBlock, RenderX, RenderY) or drawn
    return drawn
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
