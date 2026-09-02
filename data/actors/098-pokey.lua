-- Ported from NSMB Editor 5's NSMBStageObj.cs case 98.

local gfx = {}

local function init(ctx)
    gfx.body = ctx.loadTexture("enemy/sanbo_body.nsbmd")
    gfx.head = ctx.loadTexture("enemy/sanbo_head.nsbmd")
end

-- The stack grows upward from the actor's tile, so the box is as tall as the
-- segment count the height field asks for.
local function grabBounds(actor)
    local height = 16 * actor.fields.height + 9
    return { x = actor.pixelX, y = actor.pixelY + 16 - height, width = 18, height = height }
end

local function render(actor, renderer)
    local y = actor.pixelY
    local drawn = false
    for _ = 2, actor.fields.height do
        drawn = renderer:draw(gfx.body, { x = actor.pixelX, y = y, width = 18, height = 16 }) or drawn
        y = y - 16
    end
    return renderer:draw(gfx.head, { x = actor.pixelX, y = y - 9, width = 18, height = 25 }) or drawn
end

return {
    objectId = 98,
    name = "Pokey",
    category = "Enemies - Desert",
    notes = "A tall cactus enemy.\n\nUnused behaviours:\n- Can be defeated by Drill Stomp",
    fields = {
        {
            key = "height",
            label = "Height",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Height formula is 2+(n mod 8) with n being the value of this nybble",
            valueOffset = 2,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
