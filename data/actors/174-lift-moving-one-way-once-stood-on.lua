-- Ported from NSMB Editor 5's NSMBStageObj.cs case 174.

local images = {}

local function init(ctx)
    images.MovingPlatformLeft = ctx.loadImage("images/MovingPlatformLeft.png")
    images.MovingPlatformRight = ctx.loadImage("images/MovingPlatformRight.png")
    images.MovingPlatformSection = ctx.loadImage("images/MovingPlatformSection.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width = 16
    width = d[5] % 0x8
    local s = 2
    if actor.objectId == 174 then
        width = d[5] % 0x10
        width = math.max(0, width - 1)
        s = 1
        x = x - 8
    end
    x = x - ((width * s + 1) * 8)
    width = (width * s + 2) * 16
    return { x = x, y = y, width = width, height = 16 }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    local w = d[5] % 0x8
    local s = 2
    if actor.objectId == 174 then
        w = d[5] % 0x10
        w = math.max(0, w - 1)
        s = 1
        RenderX = RenderX - 8
    end
    RenderX = RenderX - ((w * s + 1) * 8)
    drawn = gfx.blit(renderer, images.MovingPlatformLeft, RenderX, RenderY) or drawn
    for l = 0, (w * s) - 1 do
        RenderX = RenderX + 16
        drawn = gfx.blit(renderer, images.MovingPlatformSection, RenderX, RenderY) or drawn
    end
    drawn = gfx.blit(renderer, images.MovingPlatformRight, RenderX + 16, RenderY) or drawn
    return drawn
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
