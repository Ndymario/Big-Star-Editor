-- Imported from NSMB Editor 5 stageobjsettings.xml.
local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16, height = 16 }
end


return {
    objectId = 92,
    name = "Coin Spawner (unused actor)",
    category = "Other",
    notes = "Todo: maximum amount of coins in air before slowdown + max in air before crash\n\nVideos that show some (0-15) of the different coin speeds can be found here:\nhttps://www.youtube.com/playlist?list=PLKKqYpRH33bLx3dywJ8y43uN1PGdqOckw\ncoin amount was 5\n\nTrigger event: coins disappear after a few seconds, with no regard to event status.",
    fields = {
        {
            key = "speed_of_coins",
            label = "Speed of coins",
            bitOffset = 8,
            bitWidth = 8,
            notes = "How fast the coins are \"thrown\".",
        },
        {
            key = "no_of_coins",
            label = "No. of coins",
            bitOffset = 0,
            bitWidth = 8,
        },
        {
            key = "spawn_nothing",
            label = "Spawn nothing",
            bitOffset = 28,
            bitWidth = 4,
            notes = "Seems to cause it to spawn nothing.",
            checkboxValue = 1,
        },
    },
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
