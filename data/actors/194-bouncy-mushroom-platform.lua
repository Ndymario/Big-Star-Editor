-- Ported from NSMB Editor 5's NSMBStageObj.cs case 194.

local images = {}

local function init(ctx)
    images.BouncingMushroom = ctx.loadImage("images/BouncingMushroom.png")
    images.BouncingMushroomStalk = ctx.loadImage("images/BouncingMushroomStalk.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    if actor.fields.shift_position == 1 then
        x = x - 8
    elseif actor.fields.shift_position == 2 then
        x = x + 8
    end
    x = x - 40
    width = 96
    height = 16 * (actor.fields.stalk_height + 2)
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    if actor.fields.shift_position == 1 then
        RenderX = RenderX - 8
    elseif actor.fields.shift_position == 2 then
        RenderX = RenderX + 8
    end
    drawn = gfx.blit(renderer, images.BouncingMushroom, RenderX - 40, RenderY) or drawn
    RenderY = RenderY + 32
    RenderX = RenderX + 3
    for l = 0, (actor.fields.stalk_height) - 1 do
        drawn = gfx.blit(renderer, images.BouncingMushroomStalk, RenderX, RenderY) or drawn
        RenderY = RenderY + 16
    end
    return drawn
end

return {
    objectId = 194,
    name = "Bouncy Mushroom Platform",
    category = "Platforms and ropes",
    notes = "No problems with underwater players bouncing on it, but doesn't appear to have special underwater behaviour.",
    fields = {
        {
            key = "stalk_height",
            label = "Stalk height",
            bitOffset = 0,
            bitWidth = 4,
        },
        {
            key = "shift_position",
            label = "Shift position",
            bitOffset = 4,
            bitWidth = 4,
            choices = {
                { value = 0, label = "No shifting" },
                { value = 1, label = "8 pixels left" },
                { value = 2, label = "8 pixels right" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
