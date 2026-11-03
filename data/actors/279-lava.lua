-- Imported from NSMB Editor 5 stageobjsettings.xml.
local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16, height = 16 }
end

-- Drawn as Liquid::renderStateCommon draws it: LiquidHandler's strip, its
-- texture the eight frames of enemy/yogan_texanim1.nsbtx, and its surface the
-- two sines of the lava entry in the LiquidAttributes table (0216c3b4).
-- Amplitudes there are fx32, so 0x1f00 is 0x1f00 / 4096 pixels.
local frames = {}
local icon

local function init(ctx)
    icon = ctx.loadImage("images/LiquidLava.png")
    for i = 0, 7 do
        frames[#frames + 1] = ctx.loadTexture("enemy/yogan_texanim1.nsbtx", { texture = i })
    end
end

local function render(actor, renderer)
    renderer:drawLiquid(frames, {
        -- renderStateCommon raises the strip 2 pixels over the actor.
        y = actor.pixelY - 2,
        frameTicks = 6,
        columnWidth = 2,
        -- waveWidth 0x0c00: the texture is stretched to 4/3 of its height.
        depthScale = 0x0c00 / 0x1000,
        waves = {
            { amplitude = 0x1f00 / 4096, step = 0x04d4, speed = 0x026a },
            { amplitude = 0x1a00 / 4096, step = 0x09d8, speed = -0x0276 },
        },
    })
    -- The canvas draws this over the liquid, so the actor can still be found and grabbed.
    return renderer:draw(icon, grabBounds(actor))
end


return {
    objectId = 279,
    name = "Lava",
    category = "Effects and decorations",
    notes = "See Water\n\nWithout either input ID, the lava will rise automatically. If you set a value for either one of the IDs by itself, the lava will rise to the height you select when that ID is activated. ID 2 (the Trigger event row) will only make the lava rise, and will wait for an off command from the ID 2 activator again. If you set values for both IDs, ID 1 (the Target event row) will make the lava go up then down.",
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
            label = "Tiles to rise",
            bitOffset = 8,
            bitWidth = 8,
        },
        {
            key = "speed_to_rise",
            label = "Speed to rise",
            bitOffset = 0,
            bitWidth = 8,
            notes = "Modulo 128. If lower than 128 then the lava will rise, if greater or equal than 128 the lava will lower.",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
