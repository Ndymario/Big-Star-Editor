-- Ported from NSMB Editor 5's NSMBStageObj.cs case 231.

local images = {}

local function init(ctx)
    images.SwellingGround = ctx.loadImage("images/SwellingGround.png")
    images.SwellingGroundIn = ctx.loadImage("images/SwellingGroundIn.png")
    images.SwellingGroundOut = ctx.loadImage("images/SwellingGroundOut.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 128
    width = 276
    height = 128
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.SwellingGround, RenderX - 128, RenderY) or drawn
    if (actor.fields.direction) == 0 then
        drawn = gfx.blit(renderer, images.SwellingGroundOut, RenderX - 128, RenderY - 112) or drawn
    elseif (actor.fields.direction) == 1 then
        drawn = gfx.blit(renderer, images.SwellingGroundIn, RenderX - 128, RenderY) or drawn
    end
    return drawn
end

return {
    objectId = 231,
    name = "Swelling ground",
    category = "Platforms and ropes",
    fields = {
        {
            key = "0_disable_1_enable_trigger_id_check",
            label = "0=Disable/1=Enable Trigger ID check",
            bitOffset = 28,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "trigger_id_1",
            label = "Trigger ID 1",
            bitOffset = 32,
            bitWidth = 8,
        },
        {
            key = "trigger_id_2",
            label = "Trigger ID 2",
            bitOffset = 40,
            bitWidth = 8,
        },
        {
            key = "direction",
            label = "Direction",
            bitOffset = 24,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Up" },
                { value = 1, label = "Down" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
