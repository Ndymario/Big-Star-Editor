-- Ported from NSMB Editor 5's NSMBStageObj.cs case 193.

local images = {}

local function init(ctx)
    images.ContractedMushroom = ctx.loadImage("images/ContractedMushroom.png")
    images.ExpandMushroomStalk = ctx.loadImage("images/ExpandMushroomStalk.png")
    images.ExpandMushroomStalkTop = ctx.loadImage("images/ExpandMushroomStalkTop.png")
    images.ExpandedMushroomL = ctx.loadImage("images/ExpandedMushroomL.png")
    images.ExpandedMushroomS = ctx.loadImage("images/ExpandedMushroomS.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    if actor.fields.start_expanded then
        if actor.fields.short_when_expanded then
            x = x - 32
            width = 80
        else
            x = x - 72
            width = 160
        end
    else
        x = x - 8
        width = 32
    end
    height = 32 + 16 * actor.fields.stalk_height
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local RenderX2, RenderY2 = RenderX, RenderY
    local drawn = false
    RenderX2 = actor.fields.stalk_height
    if actor.fields.start_expanded then
        if actor.fields.short_when_expanded then
            drawn = gfx.blit(renderer, images.ExpandedMushroomS, RenderX - 32, RenderY) or drawn
        else
            drawn = gfx.blit(renderer, images.ExpandedMushroomL, RenderX - 72, RenderY) or drawn
        end
        RenderX2 = RenderX2 + 1
        RenderY2 = RenderY2 - 16
    else
        drawn = gfx.blit(renderer, images.ContractedMushroom, RenderX - 8, RenderY) or drawn
    end
    RenderY2 = RenderY2 + 32
    for l = 0, (RenderX2) - 1 do
        if l == 0 then
            drawn = gfx.blit(renderer, images.ExpandMushroomStalkTop, RenderX, RenderY2) or drawn
        else
            drawn = gfx.blit(renderer, images.ExpandMushroomStalk, RenderX, RenderY2) or drawn
        end
        RenderY2 = RenderY2 + 16
    end
    return drawn
end

return {
    objectId = 193,
    name = "Blue Mushroom Platform (Expanding/contracting)",
    category = "Platforms and ropes",
    fields = {
        {
            key = "short_when_expanded",
            label = "Short when Expanded",
            bitOffset = 8,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "start_expanded",
            label = "Start Expanded",
            bitOffset = 4,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "stalk_height",
            label = "Stalk Height",
            bitOffset = 0,
            bitWidth = 4,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
