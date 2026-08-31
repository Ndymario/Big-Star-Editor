-- Ported from NSMB Editor 5's NSMBStageObj.cs case 157.

local images = {}

local function init(ctx)
    images.SpikeBass = ctx.loadImage("images/SpikeBass.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 3, y = actor.pixelY - 10, width = 45, height = 53 }
end

local function render(actor, renderer)
    return renderer:draw(images.SpikeBass, grabBounds(actor))
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
