-- Ported from NSMB Editor 5's NSMBStageObj.cs case 204.

local images = {}

local function init(ctx)
    images.Pump = ctx.loadImage("images/Pump.png")
    images.SwollenPipe = ctx.loadImage("images/SwollenPipe.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY - 56, width = 32, height = 72 }
end

local function render(actor, renderer)
    local pumpX = actor.pixelX + 48 + 16 * actor.fields.horizontal_distance_between_pump_and_tube
    local pumpY = actor.pixelY - 15 - 16 * actor.fields.vertical_distance_between_pump_and_tube
    if actor.fields.shift_21_tiles_left then
        pumpX = pumpX - 336
    end
    if actor.fields.shift_16_tiles_down then
        pumpY = pumpY + 256
    end

    renderer:draw(images.SwollenPipe, grabBounds(actor))
    return renderer:draw(images.Pump, { x = pumpX, y = pumpY })
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
            bitWidth = 1,
            checkboxValue = 1,
        },
        {
            key = "shift_16_tiles_down",
            label = "Shift 16 tiles down",
            bitOffset = 16,
            bitWidth = 1,
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
