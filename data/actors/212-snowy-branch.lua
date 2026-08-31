-- Ported from NSMB Editor 5's NSMBStageObj.cs case 212.

local images = {}

local function init(ctx)
    images.SnowBranchLeft = ctx.loadImage("images/SnowBranchLeft.png")
    images.SnowBranchRight = ctx.loadImage("images/SnowBranchRight.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    if actor.fields.branch_side == 1 then
        x = x - 31
        y = y - 12
        width = 34
        height = 23
    else
        x = x + 15
        y = y - 12
        width = 30
        height = 23
    end
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    if actor.fields.branch_side == 1 then
        drawn = gfx.blit(renderer, images.SnowBranchLeft, RenderX - 31, RenderY - 12) or drawn
    else
        drawn = gfx.blit(renderer, images.SnowBranchRight, RenderX + 15, RenderY - 12) or drawn
    end
    return drawn
end

return {
    objectId = 212,
    name = "Snowy branch",
    category = "Enemies - Snow",
    fields = {
        {
            key = "branch_side",
            label = "Branch side",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Right" },
                { value = 1, label = "Left" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
