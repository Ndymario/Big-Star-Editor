-- Ported from NSMB Editor 5's NSMBStageObj.cs case 174.

local images = {}

local function init(ctx)
    images.MovingPlatformLeft = ctx.loadImage("images/MovingPlatformLeft.png")
    images.MovingPlatformRight = ctx.loadImage("images/MovingPlatformRight.png")
    images.MovingPlatformSection = ctx.loadImage("images/MovingPlatformSection.png")
end

local function grabBounds(actor)
    local tiles = math.max(actor.fields.length, 2)
    return {
        x = actor.pixelX - tiles * 8,
        y = actor.pixelY,
        width = tiles * 16,
        height = 16,
    }
end

local function render(actor, renderer)
    local box = grabBounds(actor)
    local tiles = math.max(actor.fields.length, 2)
    renderer:draw(images.MovingPlatformLeft, { x = box.x, y = box.y })
    for column = 1, tiles - 2 do
        renderer:draw(images.MovingPlatformSection, { x = box.x + 16 * column, y = box.y })
    end
    return renderer:draw(images.MovingPlatformRight, { x = box.x + 16 * (tiles - 1), y = box.y })
end

return {
    objectId = 174,
    name = "Lift moving one way once stood on",
    category = "Platforms and ropes",
    fields = {
        {
            key = "direction",
            label = "Direction",
            bitOffset = 16,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Up" },
                { value = 1, label = "Down" },
                { value = 2, label = "Right" },
                { value = 3, label = "Left" },
            },
        },
        {
            key = "length",
            label = "Length",
            bitOffset = 0,
            bitWidth = 4,
            valueOffset = 1,
        },
        {
            key = "speed",
            label = "Speed",
            bitOffset = 24,
            bitWidth = 4,
            choices = {
                { value = 2, label = "15 pixels/sec" },
                { value = 1, label = "30 pixels/sec" },
                { value = 0, label = "60 pixels/sec" },
            },
        },
        {
            key = "travelling_distance",
            label = "Travelling distance",
            bitOffset = 20,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Unlimited" },
                { value = 1, label = "114 Blocks" },
                { value = 2, label = "232 Blocks" },
                { value = 3, label = "348 Blocks" },
                { value = 4, label = "464 Blocks" },
                { value = 5, label = "580 Blocks" },
                { value = 6, label = "696 Blocks" },
                { value = 7, label = "7112 Blocks" },
                { value = 8, label = "8128 Blocks" },
                { value = 9, label = "9144 Blocks" },
                { value = 10, label = "10160 Blocks" },
                { value = 15, label = "11176 Blocks" },
                { value = 16, label = "12192 Blocks" },
                { value = 17, label = "13208 Blocks" },
                { value = 18, label = "14224 Blocks" },
                { value = 19, label = "15240 Blocks" },
            },
        },
        {
            key = "enable_trigger_id_check",
            label = "Enable Trigger ID check",
            bitOffset = 8,
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
