-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 233,
    name = "Bouncy bricks (Unused)",
    category = "Platforms and ropes",
    color = "#4f86c6",
    shape = "box",
    known = true,
    complete = true,
    notes = "It is glitchy with Mini Mario, he just falls through when it wobbles. You can work around this by putting a solid underneath it.\n\n\nNOTES:\nThe texture inside puyo_lift.nsbtx is sampletex2.\nThis 'sampletex2' suggests the sprite was for testing or the texture was temorary.\n\nIt's texture is an early brick block sprite sheet",
    grabBounds = { x = -40, y = -48, width = 96, height = 64 },
    renderBounds = { x = -40, y = -48, width = 96, height = 64 },
    sourceFiles = {
        "/enemy/puyo_lift.nsbtx (jelly_lift)",
    },
    fields = {
        {
            key = "behaviour",
            label = "Behaviour",
            bitOffset = 28,
            bitWidth = 4,
            notes = "Glitchy with Mini Mario, he just falls through the bricks when they wobble.",
            choices = {
                { value = 0, label = "Stretch up & down" },
                { value = 1, label = "Depress while stood on" },
                { value = 2, label = "Solid" },
            },
        },
    },
    model = readModel("puyo_lift.nsbtx"),
}
