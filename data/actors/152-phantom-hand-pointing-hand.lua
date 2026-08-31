-- Ported from NSMB Editor 5's NSMBStageObj.cs case 152.

local images = {}

local function init(ctx)
    images.Hand = ctx.loadImage("images/Hand.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x + 8
    width = 16
    height = 20
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.Hand, RenderX + 8, RenderY) or drawn
    return drawn
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
