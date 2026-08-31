-- Ported from NSMB Editor 5's NSMBStageObj.cs case 44.

local images = {}

local function init(ctx)
    images.ChainChomp = ctx.loadImage("images/ChainChomp.png")
    images.ChainChompLog = ctx.loadImage("images/ChainChompLog.png")
end

local function grabBounds(actor)
    if actor.fields.number_of_c_chomps == 1 then
        return { x = actor.pixelX - 37, y = actor.pixelY - 48, width = 45, height = 48 }
    end
    return { x = actor.pixelX - 8, y = actor.pixelY - 48, width = 16, height = 48 }
end

local function render(actor, renderer)
    local drawn = renderer:draw(images.ChainChompLog, {
        x = actor.pixelX - 8,
        y = actor.pixelY - 48,
        width = 16,
        height = 48,
    })
    if actor.fields.number_of_c_chomps == 1 then
        return renderer:draw(images.ChainChomp, {
            x = actor.pixelX - 37,
            y = actor.pixelY - 28,
            width = 44,
            height = 28,
        }) or drawn
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
