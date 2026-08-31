-- Ported from NSMB Editor 5's NSMBStageObj.cs case 161.

local images = {}

local function init(ctx)
    images.MovingPlatformLeft = ctx.loadImage("images/MovingPlatformLeft.png")
    images.MovingPlatformLeftE = ctx.loadImage("images/MovingPlatformLeftE.png")
    images.MovingPlatformRight = ctx.loadImage("images/MovingPlatformRight.png")
    images.MovingPlatformRightE = ctx.loadImage("images/MovingPlatformRightE.png")
    images.MovingPlatformSection = ctx.loadImage("images/MovingPlatformSection.png")
    images.MovingPlatformSectionE = ctx.loadImage("images/MovingPlatformSectionE.png")
end

local function drawPlatform(renderer, x, y, length, ghost)
    local left = ghost and images.MovingPlatformLeftE or images.MovingPlatformLeft
    local middle = ghost and images.MovingPlatformSectionE or images.MovingPlatformSection
    local right = ghost and images.MovingPlatformRightE or images.MovingPlatformRight
    local drawn = renderer:draw(left, { x = x, y = y })
    for section = 2, length - 1 do
        drawn = renderer:draw(middle, { x = x + 16 * (section - 1), y = y }) or drawn
    end
    return renderer:draw(right, { x = x + 16 * (length - 1), y = y }) or drawn
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16 * math.max(actor.fields.length, 2), height = 16 }
end

local function render(actor, renderer)
    local distance = actor.fields.tiles_to_move * 16
    if actor.fields.starting_direction == 1 then
        distance = -distance
    end
    local target_y = actor.pixelY - distance
    local length = math.max(actor.fields.length, 2)
    local center = length * 8
    local drawn = renderer:drawLine({ x = actor.pixelX + center, y = actor.pixelY + 8, x2 = actor.pixelX + center, y2 = target_y + 8, color = "#ffffff", strokeWidth = 1 })
    drawn = renderer:fillEllipse(gfx.colored({ x = actor.pixelX + center - 4, y = target_y + 4, width = 7, height = 7 }, "#ffffff")) or drawn
    drawn = drawPlatform(renderer, actor.pixelX, actor.pixelY, length, false) or drawn
    if target_y ~= actor.pixelY then
        drawn = drawPlatform(renderer, actor.pixelX, target_y, length, true) or drawn
    end
    return drawn
end

return {
    objectId = 161,
    name = "Lift moving up and down",
    category = "Platforms and ropes",
    fields = {
        {
            key = "tiles_to_move",
            label = "Tiles to move",
            bitOffset = 8,
            bitWidth = 4,
        },
        {
            key = "starting_direction",
            label = "Starting direction",
            bitOffset = 16,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Up" },
                { value = 1, label = "Down" },
            },
        },
        {
            key = "length",
            label = "Length",
            bitOffset = 0,
            bitWidth = 3,
            notes = "Length formula is 1+(N11 mod 8).",
            valueOffset = 1,
        },
        {
            key = "slower",
            label = "Slower",
            bitOffset = 24,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "enable_trigger_id",
            label = "Enable trigger ID",
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
