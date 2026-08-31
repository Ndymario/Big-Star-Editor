-- Ported from NSMB Editor 5's NSMBStageObj.cs case 30.

local gfx = {}
local function init(ctx)
    gfx.left = ctx.loadImage("images/CheepCheepTrailLeft.png")
    gfx.right = ctx.loadImage("images/CheepCheepTrailRight.png")
end

local function grabBounds(actor)
    local x = actor.fields.swims_left and actor.pixelX - 1 or actor.pixelX - 62
    return { x = x, y = actor.pixelY + 4, width = 80, height = 29 }
end

local function render(actor, renderer)
    local box = grabBounds(actor)
    local image = actor.fields.swims_left and gfx.left or gfx.right
    return renderer:draw(image, box)
end

return {
    objectId = 30,
    name = "Coin trail Cheep Cheep",
    category = "Enemies - Water",
    notes = "Unused.",
    fields = {
        {
            key = "swims_left",
            label = "Swims left",
            bitOffset = 28,
            bitWidth = 4,
            notes = "Unused.",
            checkboxValue = 8,
        },
        {
            key = "avoids_mario",
            label = "Avoids Mario",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Also swims faster. Unused.",
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
