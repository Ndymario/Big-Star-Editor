-- Ported from NSMB Editor 5's NSMBStageObj.cs case 112.

local images = {}

local function init(ctx)
    images.CheepChomp = ctx.loadImage("images/CheepChomp.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    width = 60
    height = 47
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.CheepChomp, RenderX, RenderY) or drawn
    return drawn
end

return {
    objectId = 112,
    name = "Cheep Chomp",
    category = "Enemies - Water",
    fields = {
        {
            key = "eat",
            label = "Eat",
            bitOffset = 40,
            bitWidth = 4,
            notes = "Do not shoot fire when it is about to eat you or the game will block, it is one of the game's fails",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
