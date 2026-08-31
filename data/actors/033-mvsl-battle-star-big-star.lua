-- Ported from NSMB Editor 5's NSMBStageObj.cs case 33.

local images = {}

local function init(ctx)
    images.VSBattleStar = ctx.loadImage("images/VSBattleStar.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 32, height = 32 }
end

local function render(actor, renderer)
    return renderer:draw(images.VSBattleStar, grabBounds(actor))
end

return {
    objectId = 33,
    name = "MvsL Battle Star (Big Star)",
    category = "Other",
    fields = {
        {
            key = "star_id",
            label = "Star ID",
            bitOffset = 0,
            bitWidth = 8,
            notes = "These must be the same number of stars as in the original level. Each star must have a unique ID, starting from 1. If not, the game will hang. Also, if the ID is set to 0, it doesn't spawn, but the game doesn't crash (as seen in SMB 1-1)",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
