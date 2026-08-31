-- Ported from NSMB Editor 5's NSMBStageObj.cs case 262.

local images = {}

local function init(ctx)
    images.RotatingCannon = ctx.loadImage("images/RotatingCannon.png")
    images.RotatingCannonEmpty = ctx.loadImage("images/RotatingCannonEmpty.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    width = 23
    height = ((d[3] % 0x8) + 1) * 16
    y = y - (height - 16)
    x = x - 3
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local img
    local drawn = false
    img = images.RotatingCannon
    img = gfx.transform(img, 0, true, false)
    for l = 0, d[3] % 0x8 do
        if (d[4] & (1 << l)) > 0 then
            drawn = gfx.blit(renderer, images.RotatingCannonEmpty, RenderX + 2, RenderY) or drawn
        elseif (d[5] & (1 << l)) > 0 then
            drawn = gfx.blit(renderer, img, RenderX - 3, RenderY) or drawn
        else
            drawn = gfx.blit(renderer, images.RotatingCannon, RenderX + 1, RenderY) or drawn
        end
        RenderY = RenderY - 16
    end
    return drawn
end

return {
    objectId = 262,
    name = "Bill Blaster Turret (Direction changing Bullet Bill launcher)",
    category = "Enemies - Castle",
    notes = "First checkboxes are bottom cannons.",
    fields = {
        {
            key = "empty_spots",
            label = "Empty spots",
            bitOffset = 8,
            bitWidth = 8,
            notes = "individual bits indicate empty spots",
        },
        {
            key = "cannons_to_flip",
            label = "Cannons to flip",
            bitOffset = 0,
            bitWidth = 8,
            notes = "individual bits indicate flipped cannons",
        },
        {
            key = "height",
            label = "Height",
            bitOffset = 16,
            bitWidth = 4,
            notes = "Height is 1 + (N7 mod 8)",
            valueOffset = 1,
        },
        {
            key = "delayed_firing",
            label = "Delayed firing",
            bitOffset = 28,
            bitWidth = 4,
            notes = "Unused.",
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
