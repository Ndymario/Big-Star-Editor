-- Ported from NSMB Editor 5's NSMBStageObj.cs case 42.

local images = {}

local function init(ctx)
    images.BalloonBoo = ctx.loadImage("images/BalloonBoo.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 53
    y = y - 106
    width = 109
    height = 108
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.BalloonBoo, RenderX - 53, RenderY - 106) or drawn
    return drawn
end

return {
    objectId = 42,
    name = "Balloon Boo",
    category = "Enemies - Ghost House",
    notes = "As far as I know, only killable with a star, though I guess if it runs out of air it dies.",
    fields = {
        {
            key = "stays_in_zone",
            label = "Stays in Zone",
            bitOffset = 16,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "zone_id",
            label = "Zone ID",
            bitOffset = 8,
            bitWidth = 8,
        },
        {
            key = "trigger_id_1",
            label = "Trigger ID 1",
            bitOffset = 32,
            bitWidth = 8,
            notes = "When waiting to be triggered, the area below Balloon Boo hurts Mario. You can't see anything there, but the game sees it as part of the sprite. When you activate the input ID, the area goes away, and Balloon Boo goes into a weird (unused or just glitchy?) state [in E version].",
        },
        {
            key = "trigger_id_2",
            label = "Trigger ID 2",
            bitOffset = 40,
            bitWidth = 8,
        },
        {
            key = "behaviour",
            label = "Behaviour",
            bitOffset = 4,
            bitWidth = 4,
            notes = "Unused, the Balloon Boo will act exactly like an original Boo, but the hitbox will be glitchy, so it is not generally recommended for use.",
            choices = {
                { value = 0, label = "Default" },
                { value = 1, label = "Acts like a Boo" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
