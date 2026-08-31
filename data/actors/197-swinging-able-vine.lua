-- Ported from NSMB Editor 5's NSMBStageObj.cs case 197.

local images = {}

local function init(ctx)
    images.Vine = ctx.loadImage("images/Vine.png")
    images.VineBottom = ctx.loadImage("images/VineBottom.png")
    images.VineTop = ctx.loadImage("images/VineTop.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    local VineH = math.min(actor.fields.length, 4)
    x = x - 16
    width = 32
    height = 16 * (VineH + 4)
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    RenderX = RenderX - 16
    local VineMax = math.min(actor.fields.length, 4)
    drawn = gfx.blit(renderer, images.VineTop, RenderX, RenderY) or drawn
    for l = 0, (VineMax + 2) - 1 do
        RenderY = RenderY + 16
        drawn = gfx.blit(renderer, images.Vine, RenderX, RenderY) or drawn
    end
    drawn = gfx.blit(renderer, images.VineBottom, RenderX, RenderY + 16) or drawn
    return drawn
end

return {
    objectId = 197,
    name = "Swinging/able vine",
    category = "Platforms and ropes",
    notes = "Requires BOTH sprite sets \"Vine\" and \"Rope\".\nIf not, it crashes.",
    fields = {
        {
            key = "swing_angle",
            label = "Swing angle",
            bitOffset = 4,
            bitWidth = 4,
        },
        {
            key = "length",
            label = "Length",
            bitOffset = 0,
            bitWidth = 4,
        },
        {
            key = "dont_restrict_angle",
            label = "Dont restrict angle",
            bitOffset = 24,
            bitWidth = 4,
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
