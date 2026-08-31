-- Ported from NSMB Editor 5's NSMBStageObj.cs case 49.

local images = {}

local function init(ctx)
    images.Infinity = ctx.loadImage("images/Infinity.png")
    images.Zoom1 = ctx.loadImage("images/Zoom1.png")
    images.Zoom2 = ctx.loadImage("images/Zoom2.png")
    images.arrow_down = ctx.loadImage("images/arrow_down.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    local ZoomLength = actor.fields.zone_width
    local ZoomHeight = math.max(actor.fields.zone_height, 1)
    local ZoomOffset = d[5] >> 5
    if ZoomLength > ZoomOffset or ZoomLength == 0 then
        if ZoomLength == 0 then
            ZoomLength = 8
        end
        x = x - (16 * ZoomLength)
        y = y - (16 * ZoomHeight)
        width = ZoomLength * 32
        height = ZoomHeight * 16
    else
        width = 72
        height = 28
    end
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local width, height = 0, 0
    local img, img2
    local drawn = false
    local ZoomLength = actor.fields.zone_width
    local ZoomHeight = actor.fields.zone_height
    local ZoomOffset = d[5] >> 5
    if ZoomLength > ZoomOffset or ZoomLength == 0 then
        local ZoomLength2 = ZoomLength
        local ZoomHeight2 = math.max(ZoomHeight, 1)
        if ZoomLength == 0 then
            ZoomLength2 = 8
        end
        for i = 0, (ZoomHeight2) - 1 do
            local ZoomY = RenderY - 16 * (i + 1)
            for j = ZoomOffset, (ZoomLength2) - 1 do
                local ZoomX1 = RenderX - 16 * (j + 1)
                local ZoomX2 = RenderX + 16 * j
                drawn = gfx.blit(renderer, images.Zoom1, ZoomX1, ZoomY) or drawn
                drawn = gfx.blit(renderer, images.Zoom2, ZoomX2, ZoomY) or drawn
            end
        end
        if ZoomLength == 0 then
            img = images.arrow_down
            img2 = images.arrow_down
            img = gfx.transform(img, 90, false, false)
            img2 = gfx.transform(img2, 270, false, false)
            drawn = gfx.blit(renderer, img, RenderX - 144, RenderY - 8 * (ZoomHeight2 + 1), 16, 16) or drawn
            drawn = gfx.blit(renderer, img2, RenderX + 128, RenderY - 8 * (ZoomHeight2 + 1), 16, 16) or drawn
            drawn = gfx.blit(renderer, images.Infinity, RenderX - 160, RenderY - 8 * (ZoomHeight2 + 1), 16, 16) or drawn
            drawn = gfx.blit(renderer, images.Infinity, RenderX + 144, RenderY - 8 * (ZoomHeight2 + 1), 16, 16) or drawn
        end
        if ZoomHeight == 0 then
            img = images.arrow_down
            img2 = images.arrow_down
            img2 = gfx.transform(img2, 180, false, false)
            drawn = gfx.blit(renderer, img, RenderX - 8, RenderY, 16, 16) or drawn
            drawn = gfx.blit(renderer, img2, RenderX - 8, RenderY - 32, 16, 16) or drawn
            drawn = gfx.blit(renderer, images.Infinity, RenderX - 8, RenderY + 16, 16, 16) or drawn
            drawn = gfx.blit(renderer, images.Infinity, RenderX - 8, RenderY - 48, 16, 16) or drawn
        end
    else
        local r = { x = RenderX, y = RenderY, width = 72, height = 28 }
        drawn = renderer:fillRect(gfx.colored(r, "#000000")) or drawn
        renderer:drawText("Zoom", r)
    end
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
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
