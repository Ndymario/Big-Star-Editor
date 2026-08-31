-- Ported from NSMB Editor 5's NSMBStageObj.cs case 193.

local images = {}

local function init(ctx)
    images.ContractedMushroom = ctx.loadImage("images/ContractedMushroom.png")
    images.ExpandMushroomStalk = ctx.loadImage("images/ExpandMushroomStalk.png")
    images.ExpandMushroomStalkTop = ctx.loadImage("images/ExpandMushroomStalkTop.png")
    images.ExpandedMushroomL = ctx.loadImage("images/ExpandedMushroomL.png")
    images.ExpandedMushroomS = ctx.loadImage("images/ExpandedMushroomS.png")
end

local function grabBounds(actor)
    if actor.fields.start_expanded then
        if actor.fields.short_when_expanded then
            return {
                x = actor.pixelX - 32,
                y = actor.pixelY,
                width = 80,
                height = 32 + 16 * actor.fields.stalk_height,
            }
        end
        return {
            x = actor.pixelX - 72,
            y = actor.pixelY,
            width = 160,
            height = 32 + 16 * actor.fields.stalk_height,
        }
    end
    return {
        x = actor.pixelX - 8,
        y = actor.pixelY,
        width = 32,
        height = 32 + 16 * actor.fields.stalk_height,
    }
end

local function render(actor, renderer)
    local cap
    local segments = actor.fields.stalk_height
    local stalkY = actor.pixelY + 32
    if actor.fields.start_expanded then
        cap = actor.fields.short_when_expanded and images.ExpandedMushroomS or images.ExpandedMushroomL
        segments = segments + 1
        stalkY = actor.pixelY + 16
    else
        cap = images.ContractedMushroom
    end

    local box = grabBounds(actor)
    renderer:draw(cap, { x = box.x, y = box.y })
    for segment = 0, segments - 1 do
        local stalk = segment == 0 and images.ExpandMushroomStalkTop or images.ExpandMushroomStalk
        renderer:draw(stalk, { x = actor.pixelX, y = stalkY + 16 * segment })
    end
    return true
end

return {
    objectId = 193,
    name = "Blue Mushroom Platform (Expanding/contracting)",
    category = "Platforms and ropes",
    fields = {
        {
            key = "short_when_expanded",
            label = "Short when Expanded",
            bitOffset = 8,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "start_expanded",
            label = "Start Expanded",
            bitOffset = 4,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "stalk_height",
            label = "Stalk Height",
            bitOffset = 0,
            bitWidth = 4,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
