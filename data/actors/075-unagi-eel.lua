-- Ported from NSMB Editor 5's NSMBStageObj.cs case 75.

local images = {}

local function init(ctx)
    images.Eel = ctx.loadImage("images/Eel.png")
    images.EelNonMoving = ctx.loadImage("images/EelNonMoving.png")
end

local function grabBounds(actor)
    if actor.fields.stays_in_place then
        return { x = actor.pixelX + 3, y = actor.pixelY + 7, width = 25, height = 23 }
    elseif actor.fields.moves_left then
        return { x = actor.pixelX + 45, y = actor.pixelY + 3, width = 104, height = 28 }
    end
    return { x = actor.pixelX + 12, y = actor.pixelY + 3, width = 104, height = 28 }
end

local function render(actor, renderer)
    local bounds = grabBounds(actor)
    if actor.fields.stays_in_place then
        return renderer:draw(images.EelNonMoving, bounds)
    elseif actor.fields.moves_left then
        bounds.flipX = true
        return renderer:draw(images.Eel, bounds)
    elseif actor.fields.moves_right then
        return renderer:draw(images.Eel, bounds)
    end

    local drawn = renderer:fillRect(gfx.colored(bounds, "#000000"))
    renderer:drawText("Eel", bounds)
    return drawn
end

return {
    objectId = 75,
    name = "Unagi (eel)",
    category = "Enemies - Water",
    notes = "This sprite will not spawn if none of the values are checked.\nOfficially known as the 'Unagi'.",
    fields = {
        {
            key = "stays_in_place",
            label = "Stays in place",
            bitOffset = 0,
            bitWidth = 4,
            checkboxValue = 1,
            notes = "use with the Auto-scrolling controller sprite, to avoid the bug of when the unagi appears as a glich",
        },
        {
            key = "moves_left",
            label = "Moves left",
            bitOffset = 4,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "moves_right",
            label = "Moves right",
            bitOffset = 16,
            bitWidth = 4,
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
