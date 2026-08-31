-- Ported from NSMB Editor 5's NSMBStageObj.cs case 49.

local images = {}

local function init(ctx)
    images.Infinity = ctx.loadImage("images/Infinity.png")
    images.Zoom1 = ctx.loadImage("images/Zoom1.png")
    images.Zoom2 = ctx.loadImage("images/Zoom2.png")
    images.arrow_down = ctx.loadImage("images/arrow_down.png")
end

local function grabBounds(actor)
    local zoneWidth = actor.fields.zone_width
    if zoneWidth <= actor.fields.zone_offset and zoneWidth ~= 0 then
        return { x = actor.pixelX, y = actor.pixelY, width = 72, height = 28 }
    end

    local effectiveWidth = zoneWidth == 0 and 8 or zoneWidth
    local effectiveHeight = math.max(actor.fields.zone_height, 1)
    return {
        x = actor.pixelX - 16 * effectiveWidth,
        y = actor.pixelY - 16 * effectiveHeight,
        width = 32 * effectiveWidth,
        height = 16 * effectiveHeight,
    }
end

local function render(actor, renderer)
    local zoneWidth = actor.fields.zone_width
    if zoneWidth > actor.fields.zone_offset or zoneWidth == 0 then
        local effectiveWidth = zoneWidth == 0 and 8 or zoneWidth
        local effectiveHeight = math.max(actor.fields.zone_height, 1)
        local drawn = false
        for row = 1, effectiveHeight do
            local y = actor.pixelY - 16 * row
            for offset = actor.fields.zone_offset, effectiveWidth - 1 do
                drawn = renderer:draw(images.Zoom1, {
                    x = actor.pixelX - 16 * (offset + 1),
                    y = y,
                }) or drawn
                drawn = renderer:draw(images.Zoom2, {
                    x = actor.pixelX + 16 * offset,
                    y = y,
                }) or drawn
            end
        end

        if zoneWidth == 0 then
            local markerY = actor.pixelY - 8 * (effectiveHeight + 1)
            drawn = renderer:draw(images.arrow_down, {
                x = actor.pixelX - 144,
                y = markerY,
                rotation = 90,
            }) or drawn
            drawn = renderer:draw(images.arrow_down, {
                x = actor.pixelX + 128,
                y = markerY,
                rotation = 270,
            }) or drawn
            drawn = renderer:draw(images.Infinity, { x = actor.pixelX - 160, y = markerY }) or drawn
            drawn = renderer:draw(images.Infinity, { x = actor.pixelX + 144, y = markerY }) or drawn
        end

        if actor.fields.zone_height == 0 then
            drawn = renderer:draw(images.arrow_down, { x = actor.pixelX - 8, y = actor.pixelY }) or drawn
            drawn = renderer:draw(images.arrow_down, {
                x = actor.pixelX - 8,
                y = actor.pixelY - 32,
                rotation = 180,
            }) or drawn
            drawn = renderer:draw(images.Infinity, { x = actor.pixelX - 8, y = actor.pixelY + 16 }) or drawn
            drawn = renderer:draw(images.Infinity, { x = actor.pixelX - 8, y = actor.pixelY - 48 }) or drawn
        end
        return drawn
    end

    local box = grabBounds(actor)
    local drawn = renderer:fillRect(gfx.colored(box, "#000000"))
    renderer:drawText("Zoom", box)
    return drawn
end

return {
    objectId = 49,
    name = "Zoom",
    category = "Camera controllers",
    notes = "Technically the documentation for this sprite is complete, just confusing, there is a suggestion of a visual representation of the sprites effects to make it easier to understand:\nhttp://nsmbhd.net/thread/1566-visual-representation-of-sprites-84-and-245-suggestion/#23062\n\nHere are the heights you get with each width:\n24 width - 18 height",
    fields = {
        {
            key = "delay",
            label = "Delay",
            bitOffset = 24,
            bitWidth = 8,
        },
        {
            key = "zone_width",
            label = "Zone width",
            bitOffset = 20,
            bitWidth = 4,
            notes = "0=infinite.",
        },
        {
            key = "zone_height",
            label = "Zone height",
            bitOffset = 16,
            bitWidth = 4,
            notes = "0=infinite.",
        },
        {
            key = "zoom_left",
            label = "Zoom left",
            bitOffset = 8,
            bitWidth = 8,
            notes = "This value will match the width (in tiles) in-game. By default the game uses a width of 16 tiles, with this sprite you can use any width between 8 and 24 included. Don't forget the game always uses a ratio of 4 tiles in width for 3 tiles in height, so to keep the best aspect you might want to put in this field a value that is divisible by 4.",
        },
        {
            key = "zoom_right",
            label = "Zoom right",
            bitOffset = 0,
            bitWidth = 8,
            notes = "Ditto. Setting this to a value equal to nybbles 8-9 (the previous field) is recommended.",
        },
        {
            key = "zone_offset",
            label = "Zone offset",
            bitOffset = 5,
            bitWidth = 3,
            notes = "The visualization also interprets the high three bits of Zoom right as the zone's inner offset.",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
