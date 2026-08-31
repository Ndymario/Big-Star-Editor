-- Ported from NSMB Editor 5's NSMBStageObj.cs case 123.

local images = {}

local function init(ctx)
    images.RoofSpiny = ctx.loadImage("images/RoofSpiny.png")
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.RoofSpiny, RenderX, RenderY) or drawn
    return drawn
end

return {
    objectId = 123,
    name = "Roof Spiny",
    category = "Enemies - Underground",
    fields = {
        {
            key = "faster",
            label = "Faster",
            bitOffset = 8,
            bitWidth = 4,
            notes = "Unused. Likely a copypaste from the Spiny.",
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
}
