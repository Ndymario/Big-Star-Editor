-- Ported from NSMB Editor 5's NSMBStageObj.cs case 40.

local images = {}

local function init(ctx)
    images.Spiny = ctx.loadImage("images/Spiny.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width = 16
    width = 20
    return { x = x, y = y, width = width, height = 16 }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.Spiny, RenderX, RenderY) or drawn
    return drawn
end

return {
    objectId = 40,
    name = "Spiny",
    category = "Enemies - Desert",
    notes = "This sprite can be spawned by Lakitu (unused) and Lakitus spawned by the Lakitu Spawner.",
    fields = {
        {
            key = "egg_type",
            label = "Egg type",
            bitOffset = 4,
            bitWidth = 4,
            notes = "1 is used when the equivalent unused Lakitu setting is used.",
            choices = {
                { value = 0, label = "Normal" },
                { value = 1, label = "Rolling" },
            },
        },
        {
            key = "spawn_as",
            label = "Spawn as",
            bitOffset = 0,
            bitWidth = 4,
            notes = "1 is used by Lakitu and Lakithunder.",
            choices = {
                { value = 0, label = "Default" },
                { value = 1, label = "Spiny Egg" },
                { value = 2, label = "Shell" },
                { value = 3, label = "Upside-down shell" },
            },
        },
        {
            key = "faster",
            label = "Faster",
            bitOffset = 8,
            bitWidth = 4,
            notes = "Unused.",
            checkboxValue = 1,
        },
        {
            key = "unknown_8",
            label = "Unknown 8",
            bitOffset = 12,
            bitWidth = 4,
            notes = "8 and 9 are used by Lakitu and Lakithunder respectively. This is possibly a \"bounciness\" setting. Or maybe nybble 5 (if it exists) is that,",
            choices = {
                { value = 0, label = "Normal" },
                { value = 8, label = "Unknown 8" },
                { value = 9, label = "Unknown 9" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
