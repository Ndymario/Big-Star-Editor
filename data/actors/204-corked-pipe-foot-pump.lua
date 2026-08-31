-- Ported from NSMB Editor 5's NSMBStageObj.cs case 204.

local images = {}

local function init(ctx)
    images.Pump = ctx.loadImage("images/Pump.png")
    images.SwollenPipe = ctx.loadImage("images/SwollenPipe.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    y = y - 56
    width = 32
    height = 72
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    local PumpX = RenderX + (d[5] & 0xF0)
    local PumpY = RenderY - (d[4] & 0xF0)
    local PumpShiftX = (d[4] % 0x2)
    local PumpShiftY = (d[3] % 0x2)
    if PumpShiftX == 1 then
        PumpX = PumpX - 336
    end
    if PumpShiftY == 1 then
        PumpY = PumpY + 256
    end
    drawn = gfx.blit(renderer, images.SwollenPipe, RenderX, RenderY - 56) or drawn
    drawn = gfx.blit(renderer, images.Pump, PumpX + 48, PumpY - 15) or drawn
    return drawn
end

return {
    objectId = 204,
    name = "Corked Pipe + foot pump",
    category = "Other",
    fields = {
        {
            key = "destination_area",
            label = "Destination area",
            bitOffset = 20,
            bitWidth = 4,
        },
        {
            key = "destination_entrance",
            label = "Destination entrance",
            bitOffset = 0,
            bitWidth = 4,
        },
        {
            key = "horizontal_distance_between_pump_and_tube",
            label = "Horizontal distance between pump and tube",
            bitOffset = 4,
            bitWidth = 4,
        },
        {
            key = "vertical_distance_between_pump_and_tube",
            label = "Vertical distance between pump and tube",
            bitOffset = 12,
            bitWidth = 4,
        },
        {
            key = "shift_21_tiles_left",
            label = "Shift 21 tiles left",
            bitOffset = 8,
            bitWidth = 4,
            choices = {
                { value = 1, label = "Yes" },
                { value = 0, label = "No" },
            },
        },
        {
            key = "shift_16_tiles_down",
            label = "Shift 16 tiles down",
            bitOffset = 16,
            bitWidth = 4,
            choices = {
                { value = 1, label = "Yes" },
                { value = 0, label = "No" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
