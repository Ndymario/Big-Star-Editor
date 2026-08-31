-- Ported from NSMB Editor 5's NSMBStageObj.cs case 29.

local gfx = {}
local function init(ctx)
    gfx.plain = ctx.loadImage("images/CheepCheepGiant.png")
    gfx.green = ctx.loadImage("images/CheepCheepGiantGreen.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 20, y = actor.pixelY - 32, width = 63, height = 52 }
end

local function render(actor, renderer)
    local image = actor.fields.green_chases_mario == 1 and gfx.green or gfx.plain
    return renderer:draw(image, { x = actor.pixelX - 20, y = actor.pixelY - 32, width = 63, height = 52 })
end

return {
    objectId = 29,
    name = "Big Cheep Cheep",
    category = "Enemies - Water",
    notes = "Unused behaviours:\n- Can be defeated by ground pound for 200pts, with no effect [may be default behaviour]",
    fields = {
        {
            key = "slower",
            label = "Slower",
            bitOffset = 12,
            bitWidth = 4,
            notes = "Unused.",
            checkboxValue = 1,
        },
        {
            key = "green_chases_mario",
            label = "Green (chases Mario)",
            bitOffset = 0,
            bitWidth = 4,
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
