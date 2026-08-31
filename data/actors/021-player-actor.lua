-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 21,
    name = "Player Actor",
    category = "Other",
    color = "#eeB24b",
    shape = "diamond",
    known = true,
    complete = false,
    grabBounds = { x = -12, y = -24, width = 24, height = 24 },
    renderBounds = { x = -12, y = -24, width = 24, height = 24 },
    fields = {
        {
            key = "character",
            label = "Character",
            bitOffset = 4,
            bitWidth = 4,
            notes = "Only works for Mario Vs Luigi,\r\nTo make the sprite compatible with \"play as Luigi\" just add another sprite with Luigi as character and Mario as controller.",
            choices = {
                { value = 0, label = "Mario" },
                { value = 1, label = "Luigi" },
            },
        },
        {
            key = "controlled_by",
            label = "Controlled by:",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Must be Mario on singleplayer, or unexpected stuff can happen!",
            choices = {
                { value = 0, label = "Mario" },
                { value = 1, label = "Luigi" },
            },
        },
    },
}
