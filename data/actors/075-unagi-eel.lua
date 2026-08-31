-- Ported from NSMB Editor 5's NSMBStageObj.cs case 75.

local images = {}

local function init(ctx)
    images.Eel = ctx.loadImage("images/Eel.png")
    images.EelNonMoving = ctx.loadImage("images/EelNonMoving.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    if d[5] % 0x10 == 1 then
        x = x + 3
        y = y + 7
        width = 25
        height = 23
    elseif d[5] // 0x10 == 1 then
        x = x + 45
        y = y + 3
        width = 104
        height = 28
    else
        x = x + 12
        y = y + 3
        width = 104
        height = 28
    end
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local width, height = 0, 0
    local img
    local drawn = false
    if d[5] % 0x10 == 1 then
        drawn = gfx.blit(renderer, images.EelNonMoving, RenderX + 3, RenderY + 7) or drawn
    elseif d[5] // 0x10 == 1 then
        img = images.Eel
        img = gfx.transform(img, 0, true, false)
        drawn = gfx.blit(renderer, img, RenderX + 45, RenderY + 3) or drawn
    elseif d[3] % 0x10 == 1 then
        drawn = gfx.blit(renderer, images.Eel, RenderX + 12, RenderY + 3) or drawn
    else
        local r = { x = RenderX + 12, y = RenderY + 3, width = 104, height = 28 }
        drawn = renderer:fillRect(gfx.colored(r, "#000000")) or drawn
        renderer:drawText("Eel", r)
    end
    return drawn
end

return {
    objectId = 75,
    name = "Unagi (eel)",
    category = "Enemies - Water",
    notes = "This sprite will not spawn if none of the values are checked.\nOfficially known as the 'Unagi'.",
    fields = {
        {
            key = "direction",
            label = "Direction",
            bitOffset = 0,
            bitWidth = 20,
            notes = "use with the Auto-scrolling controller sprite, to avoid the bug of when the unagi appears as a glich",
            choices = {
                { value = 0, label = "Doesn't spawn" },
                { value = 1, label = "Stays in place" },
                { value = 16, label = "Left" },
                { value = 65536, label = "Right" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
