-- Ported from NSMB Editor 5's NSMBStageObj.cs case 172.

local images = {}

local function init(ctx)
    images.ScalePlatformBolt = ctx.loadImage("images/ScalePlatformBolt.png")
    images.ScalePlatformEnd = ctx.loadImage("images/ScalePlatformEnd.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 24
    y = y - 10
    width = 16 * actor.fields.distance_between_steps + 48
    height = 16 * math.max(actor.fields.right_side_rope_height, actor.fields.left_side_rope_height) + 18
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local RenderX2, RenderY2 = RenderX, RenderY
    local drawn = false
    RenderX2 = RenderX + 16 * actor.fields.distance_between_steps - 10
    drawn = gfx.blit(renderer, images.ScalePlatformBolt, RenderX - 2, RenderY - 10) or drawn
    drawn = gfx.blit(renderer, images.ScalePlatformBolt, RenderX2, RenderY - 10) or drawn
    local rope = { color = "#31184a", strokeWidth = 1 }
    drawn = renderer:drawLine({ x = RenderX + 8, y = RenderY - 9, x2 = RenderX2 + 2, y2 = RenderY - 9, color = "#ffffff", strokeWidth = 1 }) or drawn
    drawn = renderer:drawLine({ x = RenderX + 9, y = RenderY - 8, x2 = RenderX2 + 1, y2 = RenderY - 8, color = rope.color, strokeWidth = rope.strokeWidth }) or drawn
    RenderY2 = RenderY + 16 * actor.fields.left_side_rope_height - 8
    local RenderY3 = RenderY + 16 * actor.fields.right_side_rope_height - 8
    drawn = renderer:drawLine({ x = RenderX - 1, y = RenderY, x2 = RenderX - 1, y2 = RenderY2 - 1, color = "#ffffff", strokeWidth = 1 }) or drawn
    drawn = renderer:drawLine({ x = RenderX, y = RenderY + 1, x2 = RenderX, y2 = RenderY2 - 1, color = rope.color, strokeWidth = rope.strokeWidth }) or drawn
    drawn = renderer:drawLine({ x = RenderX2 + 9, y = RenderY + 3, x2 = RenderX2 + 9, y2 = RenderY3 - 1, color = "#ffffff", strokeWidth = 1 }) or drawn
    drawn = renderer:drawLine({ x = RenderX2 + 10, y = RenderY + 1, x2 = RenderX2 + 10, y2 = RenderY3 - 1, color = rope.color, strokeWidth = rope.strokeWidth }) or drawn
    drawn = gfx.blit(renderer, images.ScalePlatformEnd, RenderX2 - 14, RenderY3) or drawn
    drawn = gfx.blit(renderer, images.ScalePlatformEnd, RenderX - 24, RenderY2) or drawn
    return drawn
end

return {
    objectId = 172,
    name = "Scale lift",
    category = "Platforms and ropes",
    fields = {
        {
            key = "left_side_rope_height",
            label = "Left side rope height",
            bitOffset = 8,
            bitWidth = 4,
        },
        {
            key = "right_side_rope_height",
            label = "Right side rope height",
            bitOffset = 4,
            bitWidth = 4,
        },
        {
            key = "distance_between_steps",
            label = "Distance between steps",
            bitOffset = 0,
            bitWidth = 4,
        },
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
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
