-- Ported from NSMB Editor 5's NSMBStageObj.cs case 42.

local images = {}

local function init(ctx)
    images.BalloonBoo = ctx.loadImage("images/BalloonBoo.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 53, y = actor.pixelY - 106, width = 109, height = 108 }
end

local function render(actor, renderer)
    return renderer:draw(images.BalloonBoo, grabBounds(actor))
end

return {
    objectId = 42,
    name = "Balloon Boo",
    category = "Enemies - Ghost House",
    notes = "As far as I know, only killable with a star, though I guess if it runs out of air it dies.\n\nTarget event: while it waits to be triggered, the area below Balloon Boo hurts Mario. You can't see anything there, but the game sees it as part of the sprite. Activating the event makes the area go away, and Balloon Boo goes into a weird (unused or just glitchy?) state [in E version].",
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
