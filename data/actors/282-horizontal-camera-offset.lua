-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 282,
    name = "Horizontal camera offset",
    category = "Camera controllers",
    color = "#3f9ca8",
    shape = "box",
    known = true,
    complete = true,
    notes = "You must set values for right and below tiles distances to activate. These are just like zones, the tiles are coordinates of activation. Setting either setting to zero will not give Mario an area to activate the sprite.\n\nThe default game lag is 2 tiles. This is in addition to that lag. This will shift Mario's center to one side or the other.\n\nCamera will not return to normal until re-entering sprite zone. So far, at least until more data is found.",
    grabBounds = { x = 0, y = 0, width = 16, height = 16 },
    renderBounds = { x = 0, y = 0, width = 16, height = 16 },
    fields = {
        {
            key = "pixel_zone_below",
            label = "Pixel zone below",
            bitOffset = 16,
            bitWidth = 8,
            notes = "This is how many pixels below the sprite that the sprite can be activated in.",
        },
        {
            key = "pixels_to_full_offset",
            label = "Pixels to full offset",
            bitOffset = 8,
            bitWidth = 8,
            notes = "This is how many pixels Mario needs to move past the sprite until it reaches the full horizontal offset.",
        },
        {
            key = "pixel_offset_left",
            label = "Pixel offset +=left",
            bitOffset = 0,
            bitWidth = 8,
            notes = "Default value is 2 tiles (32 pixels) either way from center",
            signed = true,
        },
    },
}
