-- Ported from NSMB Editor 5's NSMBStageObj.cs case 136.

local images = {}

local function init(ctx)
    images.CheepCheep = ctx.loadImage("images/CheepCheep.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width = 16
    width = 21
    return { x = x, y = y, width = width, height = 16 }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.CheepCheep, RenderX, RenderY) or drawn
    return drawn
end

return {
    objectId = 136,
    name = "Jumping Cheep Cheep",
    category = "Enemies - Water",
    notes = "Funny (but useful) Glitch:\nIf you put a Green Cheep Cheep on the same screen as one of these, it becomes green!",
    fields = {
        {
            key = "unknown_10",
            label = "Unknown 10",
            bitOffset = 4,
            bitWidth = 4,
            notes = "1=Stays on screen and only jumps when the boss tells it to. Used in Cheepskipper battle. 2=Also used for some of them the in Cheepskipper battle, purpose unkonwn",
            choices = {
                { value = 0, label = "Normal" },
                { value = 1, label = "Unknown 1" },
                { value = 2, label = "Unknown 2" },
            },
        },
        {
            key = "unknown_11",
            label = "Unknown 11",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Set to 1 for some of them in the Cheepskipper battle.",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
