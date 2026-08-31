-- Ported from NSMB Editor 5's NSMBStageObj.cs case 86.

local images = {}

local function init(ctx)
    images.SwitchBlock = ctx.loadImage("images/SwitchBlock.png")
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.SwitchBlock, RenderX, RenderY) or drawn
    return drawn
end

return {
    objectId = 86,
    name = "Event trigger block",
    category = "Event controllers",
    notes = "Can only be activated by jump-from-below or Ground Pound.\nSwitches an event on, then when hit again; off, then when hit again; on....etc",
    fields = {
        {
            key = "target_id",
            label = "Target ID",
            bitOffset = 32,
            bitWidth = 8,
        },
    },
    init = init,
    render = render,
}
