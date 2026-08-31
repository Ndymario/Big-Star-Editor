-- Ported from NSMB Editor 5's NSMBStageObj.cs case 198.

local images = {}

local function init(ctx)
    images.DownArrow = ctx.loadImage("images/DownArrow.png")
    images.ManualPlatform = ctx.loadImage("images/ManualPlatform.png")
    images.UpArrow = ctx.loadImage("images/UpArrow.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    width = 112
    height = 16
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.ManualPlatform, RenderX, RenderY) or drawn
    if actor.fields.directions_arrows == 0 or actor.fields.directions_arrows == 2 then
        drawn = gfx.blit(renderer, images.UpArrow, RenderX + 51, RenderY + 4) or drawn
    elseif actor.fields.directions_arrows == 1 then
        drawn = gfx.blit(renderer, images.DownArrow, RenderX + 51, RenderY + 4) or drawn
    end
    return drawn
end

return {
    objectId = 198,
    name = "Balance lift",
    category = "Platforms and ropes",
    notes = "Stops moving when it gets near to the top of the view.",
    fields = {
        {
            key = "loads_from",
            label = "Loads from",
            bitOffset = 20,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Level start" },
                { value = 1, label = "Midway point" },
            },
        },
        {
            key = "disappears",
            label = "Disappears",
            bitOffset = 12,
            bitWidth = 4,
            notes = "Disappears after a few seconds. 1 and 2 are unused.",
            choices = {
                { value = 0, label = "No" },
                { value = 1, label = "Yes" },
                { value = 2, label = "When Mario's off" },
            },
        },
        {
            key = "starts_w_o_player",
            label = "Starts w/o player",
            bitOffset = 4,
            bitWidth = 4,
            checkboxValue = 2,
        },
        {
            key = "directions_arrows",
            label = "Directions/Arrows",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Left Up Right" },
                { value = 1, label = "Left  Down Right" },
                { value = 2, label = "Left Up Right" },
                { value = 3, label = "Left Right" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
