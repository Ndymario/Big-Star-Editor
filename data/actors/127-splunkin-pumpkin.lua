-- Ported from NSMB Editor 5's NSMBStageObj.cs case 127.

local images = {}

local function init(ctx)
    images.Pumpkin = ctx.loadImage("images/Pumpkin.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    y = y - 2
    width = 18
    height = 18
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.Pumpkin, RenderX, RenderY - 2) or drawn
    return drawn
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
