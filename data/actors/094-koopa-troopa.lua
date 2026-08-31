-- Ported from NSMB Editor 5's NSMBStageObj.cs case 94.

local images = {}

local function init(ctx)
    images.BlueKoopaShell = ctx.loadImage("images/BlueKoopaShell.png")
    images.GreenKoopaShell = ctx.loadImage("images/GreenKoopaShell.png")
    images.KoopaBlue = ctx.loadImage("images/KoopaBlue.png")
    images.KoopaGreen = ctx.loadImage("images/KoopaGreen.png")
    images.KoopaRed = ctx.loadImage("images/KoopaRed.png")
    images.RedKoopaShell = ctx.loadImage("images/RedKoopaShell.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local height = 16
    if not actor.fields.empty_shell then
        y = y - 13
        height = 29
    end
    return { x = x, y = y, width = 16, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local img
    local drawn = false
    local inShell = actor.fields.empty_shell
    if actor.fields.color == 1 then
        img = (inShell and images.RedKoopaShell or images.KoopaRed)
    elseif actor.fields.color == 2 or actor.fields.color == 3 then
        img = (inShell and images.BlueKoopaShell or images.KoopaBlue)
    else
        img = (inShell and images.GreenKoopaShell or images.KoopaGreen)
    end
    if inShell then
        drawn = gfx.blit(renderer, img, RenderX, RenderY) or drawn
    else
        drawn = gfx.blit(renderer, img, RenderX, RenderY - 13) or drawn
    end
    return drawn
end

return {
    objectId = 94,
    name = "Koopa Troopa",
    category = "Enemies - Overground",
    fields = {
        {
            key = "color",
            label = "Color",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Red Koopas stay on ledges, but we all knew that... did you?",
            choices = {
                { value = 0, label = "Green" },
                { value = 1, label = "Red" },
                { value = 2, label = "Blue" },
                { value = 3, label = "Blue (stays on ledge)" },
            },
        },
        {
            key = "empty_shell",
            label = "Empty shell",
            bitOffset = 4,
            bitWidth = 4,
            notes = "Only an empty shell spawns. Used by Bowser Jr. Shows unused behavior if colour is set to blue.",
            checkboxValue = 1,
        },
        {
            key = "shell_disappears",
            label = "Shell disappears",
            bitOffset = 28,
            bitWidth = 4,
            notes = "Disappears after a while if set to \"Empty shell\". Used by shell thrown by Bowser Jr.",
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
