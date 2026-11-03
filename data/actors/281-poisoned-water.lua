-- Imported from NSMB Editor 5 stageobjsettings.xml.
local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16, height = 16 }
end

-- Drawn as Liquid::renderStateCommon draws it: LiquidHandler's strip, its
-- texture the eight frames of enemy/doku_texanim1.nsbtx, and its surface the
-- two sines of the poison entry in the LiquidAttributes table (0216c3b4).
-- Amplitudes there are fx32, so 0x1900 is 0x1900 / 4096 pixels.
local frames = {}
local icon

local function init(ctx)
    icon = ctx.loadImage("images/LiquidPoison.png")
    for i = 0, 7 do
        frames[#frames + 1] = ctx.loadTexture("enemy/doku_texanim1.nsbtx", { texture = i })
    end
end

local function render(actor, renderer)
    renderer:drawLiquid(frames, {
        -- renderStateCommon raises the strip 2 pixels over the actor.
        y = actor.pixelY - 2,
        frameTicks = 4,
        columnWidth = 2,
        depthScale = 0x1000 / 0x1000,
        waves = {
            { amplitude = 0x1900 / 4096, step = 0x0400, speed = 0x0280 },
            { amplitude = 0x1900 / 4096, step = 0x0787, speed = -0x034b },
        },
    })
    -- The canvas draws this over the liquid, so the actor can still be found and grabbed.
    return renderer:draw(icon, grabBounds(actor))
end


return {
    objectId = 281,
    name = "Poisoned Water",
    category = "Effects and decorations",
    notes = "See Water",
    fields = {
        {
            key = "unknown_6",
            label = "Unknown 6",
            bitOffset = 20,
            bitWidth = 4,
            notes = "Nybbles 6 and 7 don't seem to have an effect. But they are used ingame. Nybbles 4 and 5 could still do something.",
        },
        {
            key = "unknown_7",
            label = "Unknown 7",
            bitOffset = 16,
            bitWidth = 4,
        },
        {
            key = "tiles_to_rise",
            label = "Tiles to Rise",
            bitOffset = 8,
            bitWidth = 8,
        },
        {
            key = "speed",
            label = "Speed",
            bitOffset = 0,
            bitWidth = 8,
            notes = "Modulo 128. If lower than 128 then the poisoned water will rise, if greater or equal than 128 the poisoned water will lower.",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
