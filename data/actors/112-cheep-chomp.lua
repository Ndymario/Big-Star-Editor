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
    notes = "Cheep Chomp reads no sprite data: neither the settings word nor either event byte changes what it does. Don't shoot fire when it is about to eat you, or the game locks up -- one of the game's own failings.",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
