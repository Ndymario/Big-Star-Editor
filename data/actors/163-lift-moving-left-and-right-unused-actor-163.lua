-- Ported from NSMB Editor 5's NSMBStageObj.cs case 163.

local images = {}

local function init(ctx)
    images.MovingPlatformLeft = ctx.loadImage("images/MovingPlatformLeft.png")
    images.MovingPlatformLeftE = ctx.loadImage("images/MovingPlatformLeftE.png")
    images.MovingPlatformRight = ctx.loadImage("images/MovingPlatformRight.png")
    images.MovingPlatformRightE = ctx.loadImage("images/MovingPlatformRightE.png")
    images.MovingPlatformSection = ctx.loadImage("images/MovingPlatformSection.png")
    images.MovingPlatformSectionE = ctx.loadImage("images/MovingPlatformSectionE.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width = 16
    width = math.max(16 * (actor.fields.length + 1), 32)
    return { x = x, y = y, width = width, height = 16 }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local RenderX2, RenderY2 = RenderX, RenderY
    local width, height = 0, 0
    local drawn = false
    local dist = (actor.fields.tiles_to_move) * 16
    local LiftLength = (actor.fields.length)
    if actor.fields.starting_direction == 1 then
        dist = -dist
    end
    if actor.objectId == 161 then
        RenderY2 = RenderY2 - dist
        LiftLength = (d[5] % 0x8)
    else
        RenderX2 = RenderX2 + dist
    end
    width = math.max(16 * (LiftLength + 1), 32) // 2
    drawn = renderer:drawLine({ x = RenderX + width, y = RenderY + 8, x2 = RenderX2 + width, y2 = RenderY2 + 8, color = "#ffffff", strokeWidth = 1 }) or drawn
    drawn = renderer:fillEllipse(gfx.colored({ x = RenderX2 + width - 4, y = RenderY2 + 4, width = 7, height = 7 }, "#ffffff")) or drawn
    drawn = gfx.blit(renderer, images.MovingPlatformLeft, RenderX, RenderY) or drawn
    for l = 0, (LiftLength - 1) - 1 do
        RenderX = RenderX + 16
        drawn = gfx.blit(renderer, images.MovingPlatformSection, RenderX, RenderY, 16, 16) or drawn
    end
    drawn = gfx.blit(renderer, images.MovingPlatformRight, RenderX + 16, RenderY) or drawn
    RenderX = (actor.pixelX / 16) * 16
    RenderY = (actor.pixelY / 16) * 16
    if RenderX ~= RenderX2 or RenderY ~= RenderY2 then
        drawn = gfx.blit(renderer, images.MovingPlatformLeftE, RenderX2, RenderY2) or drawn
        for l = 0, (LiftLength - 1) - 1 do
            RenderX2 = RenderX2 + 16
            drawn = gfx.blit(renderer, images.MovingPlatformSectionE, RenderX2, RenderY2) or drawn
        end
        drawn = gfx.blit(renderer, images.MovingPlatformRightE, RenderX2 + 16, RenderY2) or drawn
    end
    return drawn
end

return {
    objectId = 163,
    name = "Lift moving left and right (unused actor 163)",
    category = "Platforms and ropes",
    notes = "Same settings as Actor 162. Does it differ in any way?",
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
                { value = 0, label = "Right" },
                { value = 1, label = "Left" },
            },
        },
        {
            key = "length",
            label = "Length",
            bitOffset = 0,
            bitWidth = 4,
        },
        {
            key = "slower",
            label = "Slower",
            bitOffset = 24,
            bitWidth = 4,
            checkboxValue = 1,
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
