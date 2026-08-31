-- Ported from NSMB Editor 5's NSMBStageObj.cs case 113.

local images = {}

local function init(ctx)
    images.BowserJr = ctx.loadImage("images/BowserJr.png")
    images.BowserJrMasked = ctx.loadImage("images/BowserJrMasked.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY - 25, width = 32, height = 41 }
end

local function render(actor, renderer)
    local behaviour = actor.fields.behaviour
    local image = (behaviour == 0 or behaviour == 3) and images.BowserJrMasked or images.BowserJr
    return renderer:draw(image, grabBounds(actor))
end

return {
    objectId = 113,
    name = "Bowser Jr.",
    category = "Bosses and cutscenes",
    notes = "Values 3 and 4 are not used at all.\nValue 5 is used internally by the game in W8 Final Castle area 3.\nValue 7 is used in W8 Tower 1 area 2 (only in the European, Korean and Chinese roms).\n\nTest the unknowns :P I know for a fact that there is something that determines if Bowser Jr. will have collision to certain sprites like the drawbridge.\n\nHas weird interactions between values.",
    fields = {
        {
            key = "fireball_immunity",
            label = "Fireball immunity",
            bitOffset = 28,
            bitWidth = 8,
            notes = "Unused.",
            checkboxValue = 17,
        },
        {
            key = "battle_mode",
            label = "Battle mode",
            bitOffset = 24,
            bitWidth = 4,
            notes = "Unused in sprite. If final boss battle is chosen, Bowser Jr. will have the same \"death\" as in the final boss battle.",
            choices = {
                { value = 0, label = "Tower battle" },
                { value = 1, label = "Final boss battle" },
            },
        },
        {
            key = "unknown_7",
            label = "Unknown 7",
            bitOffset = 16,
            bitWidth = 4,
            notes = "Used in-game. Has something to do with Bowser Jr. running behavior. Mandatory if you plan to use the Invisible behaviour.",
            checkboxValue = 1,
        },
        {
            key = "speed_progression",
            label = "Speed progression",
            bitOffset = 8,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Slow-slow-med" },
                { value = 1, label = "Slow-med-med" },
                { value = 2, label = "Slow-med-fast" },
            },
        },
        {
            key = "behaviour",
            label = "Behaviour",
            bitOffset = 4,
            bitWidth = 4,
            notes = "11, 12 and 14 are unused.",
            choices = {
                { value = 2, label = "Normal" },
                { value = 1, label = "Normal (jumping)" },
                { value = 0, label = "With mask" },
                { value = 3, label = "With mask (jumping)" },
                { value = 11, label = "Normal (invisible)" },
                { value = 12, label = "Unstompable" },
                { value = 14, label = "Unstompable (invisible)" },
            },
        },
        {
            key = "doesn_t_manipulate_camera",
            label = "Doesn't manipulate camera",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Normally Bowser Jr. changes the camera if player moves out of the small area around him.",
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
