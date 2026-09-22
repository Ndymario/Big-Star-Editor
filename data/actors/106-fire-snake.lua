-- Ported from NSMB Editor 5's NSMBStageObj.cs case 106.

local images = {}

local function init(ctx)
    images.JumpingFlame = ctx.loadImage("images/JumpingFlame.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY - 7, width = 27, height = 23 }
end

local function render(actor, renderer)
    return renderer:draw(images.JumpingFlame, grabBounds(actor))
end

return {
    objectId = 106,
    name = "Fire Snake",
    category = "Enemies - Desert",
    notes = "A snake made of fire that follows you by jumping. It's a very annoying enemy!\n\nFire Snake reads no sprite data: neither the settings word nor either event byte changes what it does. Do not use it on solid walls; the game errors.",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
