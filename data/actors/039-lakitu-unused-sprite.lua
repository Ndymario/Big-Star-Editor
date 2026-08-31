-- Ported from NSMB Editor 5's NSMBStageObj.cs case 39.

local images = {}

local function init(ctx)
    images.Lakitu = ctx.loadImage("images/Lakitu.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    y = y - 23
    width = 26
    height = 39
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.Lakitu, RenderX, RenderY - 23) or drawn
    return drawn
end

return {
    objectId = 39,
    name = "Lakitu (unused sprite)",
    category = "Enemies - Overground",
    notes = "This sprite spawns Lakitu once. Once it's killed, it will not respawn.\nIf you want it to respawn, use sprite Lakitu Spawner (183) instead. \nIf Sprite Set 5:1 (Bullet Bill/Bob-omb) isn't enabled, the game will freeze when lakitu tries to display the Spiny egg model.\n",
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
        {
            key = "correct_order",
            label = "Correct order",
            bitOffset = 40,
            bitWidth = 4,
            notes = "if you place 2 in the same place they generate a bug in the game",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
