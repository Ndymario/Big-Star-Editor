-- Ported from NSMB Editor 5's NSMBStageObj.cs case 212.

local images = {}

local function init(ctx)
    images.SnowBranchLeft = ctx.loadImage("images/SnowBranchLeft.png")
    images.SnowBranchRight = ctx.loadImage("images/SnowBranchRight.png")
end

local function grabBounds(actor)
    if actor.fields.branch_side == 1 then
        return { x = actor.pixelX - 31, y = actor.pixelY - 12, width = 34, height = 23 }
    end
    return { x = actor.pixelX + 15, y = actor.pixelY - 12, width = 30, height = 23 }
end

local function render(actor, renderer)
    local branch = actor.fields.branch_side == 1 and images.SnowBranchLeft or images.SnowBranchRight
    return renderer:draw(branch, grabBounds(actor))
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
