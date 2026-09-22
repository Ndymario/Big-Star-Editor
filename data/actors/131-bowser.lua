-- Ported from NSMB Editor 5's NSMBStageObj.cs case 131.

local images = {}

local function init(ctx)
    images.Bowser = ctx.loadImage("images/Bowser.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 41, y = actor.pixelY - 38, width = 76, height = 55 }
end

local function render(actor, renderer)
    return renderer:draw(images.Bowser, grabBounds(actor))
end

return {
    objectId = 131,
    name = "Bowser",
    category = "Bosses and cutscenes",
    notes = "- Can start moving without being activated in some circumstances [desmume glitch?]\n- After roaring it allows Mario to move after boss activator makes him stop.",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
