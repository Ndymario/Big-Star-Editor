-- Ported from NSMB Editor 5's NSMBStageObj.cs case 60, which shares case 28's drawing but never the green variant.

local plain
local function init(ctx)
    plain = ctx.loadImage("images/CheepCheep.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 1, y = actor.pixelY + 4, width = 19, height = 18 }
end

local function render(actor, renderer)
    return renderer:draw(plain, { x = actor.pixelX - 1, y = actor.pixelY + 4, width = 19, height = 18 })
end

return {
    objectId = 60,
    name = "Cheep Cheep random spawner",
    category = "Enemies - Water",
    notes = "Let spawn random generated Cheep Cheep in the selected Zone.",
    fields = {
        {
            key = "zone_id",
            label = "Zone ID",
            bitOffset = 8,
            bitWidth = 8,
            notes = "Let spawn random generated Cheep-Cheep in the selected Zone.",
        },
        {
            key = "green_chases_mario",
            label = "Green (chases Mario)",
            bitOffset = 4,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "faster_out_of_water",
            label = "Faster & out of water",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Makes some swim faster and makes them cover whole screen, not just part with water",
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
