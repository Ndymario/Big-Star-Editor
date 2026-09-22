-- Ported from NSMB Editor 5's NSMBStageObj.cs case 39.

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
    objectId = 39,
    name = "Lakitu (unused sprite)",
    category = "Enemies - Overground",
    notes = "This sprite spawns Lakitu once. Once it's killed, it will not respawn.\nIf you want it to respawn, use sprite Lakitu Spawner (183) instead. \nIf Sprite Set 5:1 (Bullet Bill/Bob-omb) isn't enabled, the game will freeze when lakitu tries to display the Spiny egg model.\n\nPlacing two in the same spot makes the game misbehave.",
    fields = {
        {
            key = "unknown_8",
            label = "Unknown 8",
            bitOffset = 12,
            bitWidth = 4,
            notes = "1 makes it throw one Spiny then go away? Set to 2 when spawned Lakitu spawner with Z order nybble set to 1.",
            choices = {
                { value = 0, label = "Normal" },
                { value = 1, label = "Unknown 1" },
                { value = 2, label = "Z order" },
            },
        },
        {
            key = "spawning_method",
            label = "Spawning method",
            bitOffset = 8,
            bitWidth = 4,
            notes = "If set, makes Lakitu come from the BG like in sprite 183.",
            choices = {
                { value = 0, label = "Normal" },
                { value = 1, label = "From background" },
            },
        },
        {
            key = "unknown_10",
            label = "Unknown 10",
            bitOffset = 4,
            bitWidth = 4,
            notes = "Set to 1 when spawned by Lakitu spawner.",
        },
        {
            key = "spiny_type",
            label = "Spiny type",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Normal" },
                { value = 1, label = "Rolling egg" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
