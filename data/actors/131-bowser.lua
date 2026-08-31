-- Ported from NSMB Editor 5's NSMBStageObj.cs case 131.

local images = {}

local function init(ctx)
    images.Bowser = ctx.loadImage("images/Bowser.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 41
    y = y - 38
    width = 76
    height = 55
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.Bowser, RenderX - 41, RenderY - 38) or drawn
    return drawn
end

return {
    objectId = 131,
    name = "Bowser",
    category = "Bosses and cutscenes",
    notes = "- Can start moving without being activated in some circumstances [desmume glitch?]\n- After roaring it allows Mario to move after boss activator makes him stop.",
    fields = {
        {
            key = "trigger_id",
            label = "Trigger ID",
            bitOffset = 32,
            bitWidth = 8,
        },
        {
            key = "bridge_id",
            label = "Bridge ID",
            bitOffset = 40,
            bitWidth = 8,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
