-- Ported from NSMB Editor 5's NSMBStageObj.cs case 149.

local images = {}

local function init(ctx)
    images.Kabomb = ctx.loadImage("images/Kabomb.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY - 3, width = 16, height = 19 }
end

local function render(actor, renderer)
    return renderer:draw(images.Kabomb, grabBounds(actor))
end

return {
    objectId = 149,
    name = "Kab-omb",
    category = "Enemies - Fire",
    fields = {
        {
            key = "chases_mario",
            label = "Chases Mario",
            bitOffset = 0,
            bitWidth = 4,
            notes = "This makes him chase Mario if hit by a fireball/Raining Debris. If not set, he will run left and right. 0 (unchecked) is unused.",
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
