-- Ported from NSMB Editor 5's NSMBStageObj.cs case 36.

local images = {}

local function init(ctx)
    images.EndingFlag = ctx.loadImage("images/EndingFlag.png")
    images.EndingFlagRed = ctx.loadImage("images/EndingFlagRed.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 29
    y = y - 119
    width = 27
    height = 24
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local img
    local drawn = false
    if actor.fields.secret_exit then
        img = images.EndingFlagRed
    else
        img = images.EndingFlag
    end
    drawn = gfx.blit(renderer, img, RenderX - 29, RenderY - 119) or drawn
    return drawn
end

return {
    objectId = 36,
    name = "Goal Pole's flag",
    category = "Bosses and cutscenes",
    notes = "AKA Flagpole Flag",
    fields = {
        {
            key = "secret_exit",
            label = "Secret exit",
            bitOffset = 28,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "upright_with_pipe_cannon",
            label = "Upright with Pipe Cannon",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Mario will grab the flag upright initially, instead of upside down, when fired from a Pipe Cannon. 1 seems to be the same as 0.",
            checkboxValue = 2,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
