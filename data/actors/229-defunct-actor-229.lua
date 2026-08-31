-- Imported from NSMB Editor 5 stageobjsettings.xml.
local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16, height = 16 }
end


return {
    objectId = 229,
    name = "Defunct Actor 229",
    category = "Useless sprites",
    notes = "Sprite set: 8-4\nReverse engineering by MeroMero:\nhttps://nsmbhd.net/thread/2966-new-actor-discoveries-and-extra-info-on-old-ones/?from=20#46117\nand Skawo:\nhttps://nsmbhd.net/thread/3820-unused-code-re/",
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
