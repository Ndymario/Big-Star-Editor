-- Ported from NSMB Editor 5's NSMBStageObj.cs case 23.

local tube

local function init(ctx)
    tube = ctx.loadImage("images/PiranhaplantTube.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY + -28, width = 32, height = 28 }
end

local function render(actor, renderer)
    return renderer:draw(tube, { x = actor.pixelX, y = actor.pixelY + -28, width = 32, height = 28 })
end

return {
    objectId = 23,
    name = "Pipe Piranha Plant (facing up)",
    category = "Enemies - Overground",
    notes = "==Unused behaviours==\n- Can be defeated by Drill Stomp\n\nNeeds to be placed on top of a [solid?]tile to work.",
    fields = {
        {
            key = "number_of_fireballs",
            label = "Number of fireballs",
            bitOffset = 0,
            bitWidth = 8,
            notes = "1+ is unused, but some are used in NSMBWii. also, test.",
            choices = {
                { value = 0, label = "0" },
                { value = 1, label = "1" },
                { value = 17, label = "2" },
                { value = 33, label = "3" },
                { value = 49, label = "6" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
