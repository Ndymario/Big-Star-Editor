-- Ported from NSMB Editor 5's NSMBStageObj.cs case 36.

local images = {}

local function init(ctx)
    images.EndingFlag = ctx.loadImage("images/EndingFlag.png")
    images.EndingFlagRed = ctx.loadImage("images/EndingFlagRed.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 29, y = actor.pixelY - 119, width = 27, height = 24 }
end

local function render(actor, renderer)
    local flag = actor.fields.secret_exit and images.EndingFlagRed or images.EndingFlag
    return renderer:draw(flag, grabBounds(actor))
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
