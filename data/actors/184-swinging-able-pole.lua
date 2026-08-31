-- Ported from NSMB Editor 5's NSMBStageObj.cs case 184.

local images = {}

local function init(ctx)
    images.LineAttachment = ctx.loadImage("images/LineAttachment.png")
    images.SwingingPole = ctx.loadImage("images/SwingingPole.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 11
    y = y - 11
    width = 22
    height = (d[5] % 0x8) * 16 + 75
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local RenderX2, RenderY2 = RenderX, RenderY
    local drawn = false
    for l = 0, (d[5] % 0x8 + 4) - 1 do
        drawn = gfx.blit(renderer, images.SwingingPole, RenderX - 3, RenderY) or drawn
        RenderY = RenderY + 16
    end
    drawn = gfx.blit(renderer, images.LineAttachment, RenderX2 - 11, RenderY2 - 11) or drawn
    return drawn
end

return {
    objectId = 184,
    name = "Swinging/able pole",
    category = "Platforms and ropes",
    fields = {
        {
            key = "height",
            label = "Height",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Height is 4+(N11 mod 8).",
            valueOffset = 4,
        },
        {
            key = "sticks_on_line",
            label = "Sticks on line",
            bitOffset = 4,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "starting_direction",
            label = "Starting direction",
            bitOffset = 8,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Right" },
                { value = 1, label = "Left" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
