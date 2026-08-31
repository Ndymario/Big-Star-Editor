-- Ported from NSMB Editor 5's NSMBStageObj.cs case 152.

local images = {}

local function init(ctx)
    images.Hand = ctx.loadImage("images/Hand.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX + 8, y = actor.pixelY, width = 16, height = 20 }
end

local function render(actor, renderer)
    return renderer:draw(images.Hand, grabBounds(actor))
end

return {
    objectId = 152,
    name = "Phantom Hand (Pointing hand)",
    category = "Enemies - Ghost House",
    notes = "Points at connected Winged ? Blocks.\nAlso points to Unused Invisible Podoboos without connections.",
    fields = {
        {
            key = "phantom_hand_id",
            label = "Phantom Hand ID",
            bitOffset = 0,
            bitWidth = 4,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
