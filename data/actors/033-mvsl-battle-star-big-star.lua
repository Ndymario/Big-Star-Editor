-- Ported from NSMB Editor 5's NSMBStageObj.cs case 33.

local images = {}

local function init(ctx)
    images.VSBattleStar = ctx.loadImage("images/VSBattleStar.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    width = 32
    height = 32
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.VSBattleStar, RenderX, RenderY) or drawn
    return drawn
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
