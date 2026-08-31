-- Ported from NSMB Editor 5's NSMBStageObj.cs case 21.

local gfx = {}

local function init(ctx)
    gfx[0] = ctx.loadImage("images/mario_small.png")
    gfx[1] = ctx.loadImage("images/luigi_small.png")
    gfx.invalid = ctx.loadImage("images/sprite_invalid.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 12, y = actor.pixelY - 24, width = 24, height = 24 }
end

local function render(actor, renderer)
    local x, y = actor.pixelX, actor.pixelY
    local drawn = false

    -- The clone character occupies the actor's own box; an unknown value keeps
    -- NSMBe's invalid marker with the number printed over it.
    local character = actor.fields.character
    if gfx[character] then
        drawn = renderer:draw(gfx[character], { x = x - 12, y = y - 24, width = 24, height = 24 })
    else
        drawn = renderer:draw(gfx.invalid, { x = x - 12, y = y - 24, width = 24, height = 24 })
        renderer:drawText(tostring(character), { x = x - (character < 10 and 6 or 10), y = y - 20 })
    end

    -- The controlling player rides along at half size in the corner.
    local controller = actor.fields.controlled_by
    if gfx[controller] then
        drawn = renderer:draw(gfx[controller], { x = x + 5, y = y - 12, width = 12, height = 12 }) or drawn
    else
        renderer:drawText(tostring(controller), { x = x + 8, y = y - 16 })
    end
    return drawn
end

return {
    objectId = 21,
    name = "Player Actor",
    category = "Other",
    fields = {
        {
            key = "character",
            label = "Character",
            bitOffset = 4,
            bitWidth = 4,
            notes = "Only works for Mario Vs Luigi,\r\nTo make the sprite compatible with \"play as Luigi\" just add another sprite with Luigi as character and Mario as controller.",
            choices = {
                { value = 0, label = "Mario" },
                { value = 1, label = "Luigi" },
            },
        },
        {
            key = "controlled_by",
            label = "Controlled by:",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Must be Mario on singleplayer, or unexpected stuff can happen!",
            choices = {
                { value = 0, label = "Mario" },
                { value = 1, label = "Luigi" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
