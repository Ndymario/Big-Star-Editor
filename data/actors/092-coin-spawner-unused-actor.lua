-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 92,
    name = "Coin Spawner (unused actor)",
    category = "Other",
    color = "#eeB24b",
    shape = "diamond",
    known = true,
    complete = true,
    notes = "Todo: maximum amount of coins in air before slowdown + max in air before crash\n\nVideos that show some (0-15) of the different coin speeds can be found here:\nhttps://www.youtube.com/playlist?list=PLKKqYpRH33bLx3dywJ8y43uN1PGdqOckw\ncoin amount was 5",
    grabBounds = { x = 0, y = 0, width = 16, height = 16 },
    renderBounds = { x = 0, y = 0, width = 16, height = 16 },
    fields = {
        {
            key = "trigger_id",
            label = "Trigger ID",
            bitOffset = 40,
            bitWidth = 8,
            notes = "Coins disappears after a few seconds, with no regard to event status.",
        },
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
}
