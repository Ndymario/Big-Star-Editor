-- Ported from NSMB Editor 5's NSMBStageObj.cs case 59.

local images = {}

local function init(ctx)
    images.Lakitu = ctx.loadImage("images/Lakitu.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY - 23, width = 26, height = 39 }
end

local function render(actor, renderer)
    return renderer:draw(images.Lakitu, grabBounds(actor))
end

return {
    objectId = 59,
    name = "Lakitu spawner",
    category = "Enemies - Overground",
    notes = "Spawns Lakitu (40).\n\nThe lakitu spawns if Mario is inside Lakitu inside the Zone. If Lakitu is killed, it is respawned. If Mario exits the Zone, Lakitu despawns.\nIf Sprite Set 5:1 (Bullet Bill/Bob-omb) isn't enabled, the game will freeze when Lakitu tries to display the Spiny egg model.\n",
    fields = {
        {
            key = "zone_id",
            label = "Zone ID",
            bitOffset = 8,
            bitWidth = 8,
        },
        {
            key = "spiny_type",
            label = "Spiny type",
            bitOffset = 4,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Normal" },
                { value = 1, label = "Rolling egg" },
            },
        },
        {
            key = "z_order",
            label = "Z Order",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Changes Z order so Spiny appear behind Lakitu when thrown. Unused.",
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
