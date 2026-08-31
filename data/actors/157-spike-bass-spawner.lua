-- Ported from NSMB Editor 5's NSMBStageObj.cs case 157.

local images = {}

local function init(ctx)
    images.SpikeBass = ctx.loadImage("images/SpikeBass.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 3
    y = y - 10
    width = 45
    height = 53
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.SpikeBass, RenderX - 3, RenderY - 10) or drawn
    return drawn
end

return {
    objectId = 157,
    name = "Spike Bass spawner",
    category = "Enemies - Water",
    notes = "Spawns a Spike Bass into onscreen actor-based liquid if the player is in the zone.\n\nTest: multiple onscreen liquids.",
    fields = {
        {
            key = "zone_id",
            label = "Zone ID",
            bitOffset = 8,
            bitWidth = 8,
        },
        {
            key = "use_restricted_movement",
            label = "Use Restricted Movement",
            bitOffset = 4,
            bitWidth = 4,
            notes = "Unused.",
            checkboxValue = 1,
        },
        {
            key = "restricted_movement_distance",
            label = "Restricted Movement Distance",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Unused.",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
