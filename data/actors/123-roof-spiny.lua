-- Ported from NSMB Editor 5's NSMBStageObj.cs case 123.

local images = {}

local function init(ctx)
    images.RoofSpiny = ctx.loadImage("images/RoofSpiny.png")
end

local function render(actor, renderer)
    return renderer:draw(images.RoofSpiny, { x = actor.pixelX, y = actor.pixelY })
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
