-- Ported from NSMB Editor 5's NSMBStageObj.cs case 112.

local images = {}

local function init(ctx)
    images.CheepChomp = ctx.loadImage("images/CheepChomp.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 60, height = 47 }
end

local function render(actor, renderer)
    return renderer:draw(images.CheepChomp, grabBounds(actor))
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
