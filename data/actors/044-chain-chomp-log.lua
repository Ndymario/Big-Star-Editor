-- Ported from NSMB Editor 5's NSMBStageObj.cs case 44.

local images = {}

local function init(ctx)
    images.ChainChomp = ctx.loadImage("images/ChainChomp.png")
    images.ChainChompLog = ctx.loadImage("images/ChainChompLog.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 8
    y = y - 48
    height = 48
    if actor.fields.number_of_c_chomps == 1 then
        x = x - 29
        width = 45
    end
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.ChainChompLog, RenderX - 8, RenderY - 48) or drawn
    if actor.fields.number_of_c_chomps == 1 then
        drawn = gfx.blit(renderer, images.ChainChomp, RenderX - 37, RenderY - 28) or drawn
    end
    return drawn
end

return {
    objectId = 44,
    name = "Chain Chomp/Log",
    category = "Enemies - Overground",
    notes = "The code has a line that crashes the game on purpose if the nybble is set to 2+. If you comment it out it seems to work fine except freeing the Chain Chomps crashes the game for an unknown reason.\nAlso crashes in VS mode.",
    fields = {
        {
            key = "number_of_c_chomps",
            label = "Number of C. Chomps",
            bitOffset = 0,
            bitWidth = 4,
            notes = "0 is unused. 2 CRASHES the game and is used in Unused Level 3, but is otherwise unused. Values above 2 also crash and are completly unused.",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
