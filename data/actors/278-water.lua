-- Imported from NSMB Editor 5 stageobjsettings.xml.
local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16, height = 16 }
end

-- Drawn as Liquid::renderStateCommon draws it: LiquidHandler's strip, its
-- texture the eight frames of enemy/suimen_texanim1.nsbtx, and its surface the
-- two sines of the water entry in the LiquidAttributes table (0216c3b4).
-- Amplitudes there are fx32, so 0x0d00 is 0x0d00 / 4096 pixels.
local frames = {}
local icon

local function init(ctx)
    icon = ctx.loadImage("images/LiquidWater.png")
    for i = 0, 7 do
        frames[#frames + 1] = ctx.loadTexture("enemy/suimen_texanim1.nsbtx", { texture = i })
    end
end

local function render(actor, renderer)
    -- Water::onCreate keeps the opacity byte only to skip drawing at 0.
    if actor.fields.opacity ~= 0 then
        renderer:drawLiquid(frames, {
            -- renderStateCommon raises the strip 2 pixels over the actor.
            y = actor.pixelY - 2,
            frameTicks = 4,
            columnWidth = 2,
            depthScale = 0x1000 / 0x1000,
            waves = {
                { amplitude = 0x0d00 / 4096, step = 0x0444, speed = 0x06ee },
                { amplitude = 0x0d00 / 4096, step = 0x0842, speed = -0x09ce },
            },
        })
    end
    -- The canvas draws this over the liquid, so the actor can still be found and grabbed.
    return renderer:draw(icon, grabBounds(actor))
end


return {
    objectId = 278,
    name = "Water",
    category = "Effects and decorations",
    notes = "There is no way to make water only appear in a zone; like in NSMBW, but you can emulate this with the unused water tiles (which has its limitations) or by quickly rising the water when you enter an area and lowering it when you exit it (with the event controllers).\n\nTarget event (ID 1): the water will rise then lower. (This is wrong, Hiccup thinks [E version].)\nTrigger event (ID 2): the water will rise, but has to be activated again with a stop command to lower.",
    fields = {
        {
            key = "water_lowering_s_delay",
            label = "Water lowering's delay",
            bitOffset = 28,
            bitWidth = 4,
            notes = "Any value other than 0 will override both Input IDs.",
        },
        {
            key = "water_rising_s_delay",
            label = "Water rising's delay",
            bitOffset = 24,
            bitWidth = 4,
            notes = "Any value other than 0 will override both Input IDs.",
        },
        {
            key = "opacity",
            label = "Opacity",
            bitOffset = 16,
            bitWidth = 5,
        },
        {
            key = "tiles_to_move",
            label = "Tiles to move",
            bitOffset = 8,
            bitWidth = 8,
        },
        {
            key = "speed",
            label = "Speed",
            bitOffset = 0,
            bitWidth = 8,
            notes = "Modulo 128. If lower than 128 then the water will rise, if greater or equal than 128 the water will lower. Lowering water does not work properly. If you want lowering water, rise it with a enter-view event, then \"unrise\" it or get the code hack that fixes the problem.",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
