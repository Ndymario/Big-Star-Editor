-- Ported from NSMB Editor 5's NSMBStageObj.cs case 127.

local images = {}

local function init(ctx)
    images.Pumpkin = ctx.loadImage("images/Pumpkin.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY - 2, width = 18, height = 18 }
end

local function render(actor, renderer)
    return renderer:draw(images.Pumpkin, grabBounds(actor))
end

return {
    objectId = 127,
    name = "Splunkin (pumpkin)",
    category = "Enemies - Ghost House",
    notes = "A walking pumpkin.",
    fields = {
        {
            key = "explodes_at_start",
            label = "Explodes at start",
            bitOffset = 0,
            bitWidth = 4,
            notes = "At spawn. Unused.",
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
