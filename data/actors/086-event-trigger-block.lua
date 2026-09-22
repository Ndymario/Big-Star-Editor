-- Ported from NSMB Editor 5's NSMBStageObj.cs case 86.

local images = {}

local function init(ctx)
    images.SwitchBlock = ctx.loadImage("images/SwitchBlock.png")
end

local function render(actor, renderer)
    return renderer:draw(images.SwitchBlock, { x = actor.pixelX, y = actor.pixelY })
end

return {
    objectId = 86,
    name = "Event trigger block",
    category = "Event controllers",
    notes = "Can only be activated by jump-from-below or Ground Pound.\nSwitches an event on, then when hit again; off, then when hit again; on....etc",
    init = init,
    render = render,
}
