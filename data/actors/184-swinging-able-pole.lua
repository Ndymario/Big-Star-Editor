-- Ported from NSMB Editor 5's NSMBStageObj.cs case 184.

local images = {}

local function init(ctx)
    images.LineAttachment = ctx.loadImage("images/LineAttachment.png")
    images.SwingingPole = ctx.loadImage("images/SwingingPole.png")
end

local function grabBounds(actor)
    return {
        x = actor.pixelX - 11,
        y = actor.pixelY - 11,
        width = 22,
        height = 16 * actor.fields.height + 11,
    }
end

local function render(actor, renderer)
    for segment = 0, actor.fields.height - 1 do
        renderer:draw(images.SwingingPole, { x = actor.pixelX - 3, y = actor.pixelY + 16 * segment })
    end
    return renderer:draw(images.LineAttachment, { x = actor.pixelX - 11, y = actor.pixelY - 11 })
end

return {
    objectId = 184,
    name = "Swinging/able pole",
    category = "Platforms and ropes",
    fields = {
        {
            key = "height",
            label = "Height",
            bitOffset = 0,
            bitWidth = 3,
            notes = "Height is 4+(N11 mod 8).",
            valueOffset = 4,
        },
        {
            key = "sticks_on_line",
            label = "Sticks on line",
            bitOffset = 4,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "starting_direction",
            label = "Starting direction",
            bitOffset = 8,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Right" },
                { value = 1, label = "Left" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
