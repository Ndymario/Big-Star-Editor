-- Ported from NSMB Editor 5's NSMBStageObj.cs case 231.

local images = {}

local function init(ctx)
    images.SwellingGround = ctx.loadImage("images/SwellingGround.png")
    images.SwellingGroundIn = ctx.loadImage("images/SwellingGroundIn.png")
    images.SwellingGroundOut = ctx.loadImage("images/SwellingGroundOut.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 128, y = actor.pixelY, width = 276, height = 128 }
end

local function render(actor, renderer)
    local box = grabBounds(actor)
    renderer:draw(images.SwellingGround, box)
    if actor.fields.direction == 0 then
        return renderer:draw(images.SwellingGroundOut, { x = box.x, y = box.y - 112 })
    elseif actor.fields.direction == 1 then
        return renderer:draw(images.SwellingGroundIn, box)
    end
    return true
end

return {
    objectId = 231,
    name = "Swelling ground",
    category = "Platforms and ropes",
    fields = {
        {
            key = "enable_trigger_id_check",
            label = "Enable trigger ID check",
            bitOffset = 28,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "direction",
            label = "Direction",
            bitOffset = 24,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Up" },
                { value = 1, label = "Down" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
