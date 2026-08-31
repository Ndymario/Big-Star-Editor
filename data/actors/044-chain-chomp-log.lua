-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 44,
    name = "Chain Chomp/Log",
    category = "Enemies - Overground",
    color = "#b5651d",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "The code has a line that crashes the game on purpose if the nybble is set to 2+. If you comment it out it seems to work fine except freeing the Chain Chomps crashes the game for an unknown reason.\nAlso crashes in VS mode.",
    grabBounds = { x = -8, y = -48, width = 16, height = 48 },
    renderBounds = { x = -8, y = -48, width = 16, height = 48 },
    sourceFiles = {
        "/enemy/w_pile.nsbtx (chain chomp's log)",
        "/enemy/wanwan.nsbmd (chain chomp)",
        "/enemy/wanwan_chain.nsbmd (chain chomp's chain)",
    },
    fields = {
        {
            key = "number_of_c_chomps",
            label = "Number of C. Chomps",
            bitOffset = 0,
            bitWidth = 4,
            notes = "0 is unused. 2 CRASHES the game and is used in Unused Level 3, but is otherwise unused. Values above 2 also crash and are completly unused.",
        },
    },
    model = readModel("wanwan.nsbmd"),
}
