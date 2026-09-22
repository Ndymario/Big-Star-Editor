-- Ported from NSMB Editor 5's NSMBStageObj.cs case 172.

local images = {}

local function init(ctx)
    images.ScalePlatformBolt = ctx.loadImage("images/ScalePlatformBolt.png")
    images.ScalePlatformEnd = ctx.loadImage("images/ScalePlatformEnd.png")
end

local function grabBounds(actor)
    return {
        x = actor.pixelX - 24,
        y = actor.pixelY - 10,
        width = 16 * actor.fields.distance_between_steps + 48,
        height = 16 * math.max(actor.fields.right_side_rope_height, actor.fields.left_side_rope_height) + 18,
    }
end

local function render(actor, renderer)
    local x, y = actor.pixelX, actor.pixelY
    local right_x = x + 16 * actor.fields.distance_between_steps - 10
    local left_y = y + 16 * actor.fields.left_side_rope_height - 8
    local right_y = y + 16 * actor.fields.right_side_rope_height - 8
    local drawn = renderer:draw(images.ScalePlatformBolt, { x = x - 2, y = y - 10 })
    drawn = renderer:draw(images.ScalePlatformBolt, { x = right_x, y = y - 10 }) or drawn
    drawn = renderer:drawLine({ x = x + 8, y = y - 9, x2 = right_x + 2, y2 = y - 9, color = "#ffffff", strokeWidth = 1 }) or drawn
    drawn = renderer:drawLine({ x = x + 9, y = y - 8, x2 = right_x + 1, y2 = y - 8, color = "#31184a", strokeWidth = 1 }) or drawn
    drawn = renderer:drawLine({ x = x - 1, y = y, x2 = x - 1, y2 = left_y - 1, color = "#ffffff", strokeWidth = 1 }) or drawn
    drawn = renderer:drawLine({ x = x, y = y + 1, x2 = x, y2 = left_y - 1, color = "#31184a", strokeWidth = 1 }) or drawn
    drawn = renderer:drawLine({ x = right_x + 9, y = y + 3, x2 = right_x + 9, y2 = right_y - 1, color = "#ffffff", strokeWidth = 1 }) or drawn
    drawn = renderer:drawLine({ x = right_x + 10, y = y + 1, x2 = right_x + 10, y2 = right_y - 1, color = "#31184a", strokeWidth = 1 }) or drawn
    drawn = renderer:draw(images.ScalePlatformEnd, { x = right_x - 14, y = right_y }) or drawn
    return renderer:draw(images.ScalePlatformEnd, { x = x - 24, y = left_y }) or drawn
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
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
