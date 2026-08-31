-- Ported from NSMB Editor 5's NSMBStageObj.cs case 233.

local images = {}

local function init(ctx)
    images.BouncyBricks = ctx.loadImage("images/BouncyBricks.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 40
    y = y - 48
    width = 96
    height = 64
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.BouncyBricks, RenderX - 40, RenderY - 48) or drawn
    return drawn
end

return {
    objectId = 233,
    name = "Bouncy bricks (Unused)",
    category = "Platforms and ropes",
    notes = "It is glitchy with Mini Mario, he just falls through when it wobbles. You can work around this by putting a solid underneath it.\n\n\nNOTES:\nThe texture inside puyo_lift.nsbtx is sampletex2.\nThis 'sampletex2' suggests the sprite was for testing or the texture was temorary.\n\nIt's texture is an early brick block sprite sheet",
    fields = {
        {
            key = "behaviour",
            label = "Behaviour",
            bitOffset = 28,
            bitWidth = 4,
            notes = "Glitchy with Mini Mario, he just falls through the bricks when they wobble.",
            choices = {
                { value = 0, label = "Stretch up & down" },
                { value = 1, label = "Depress while stood on" },
                { value = 2, label = "Solid" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
