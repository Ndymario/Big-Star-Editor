-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 86,
    name = "Event trigger block",
    category = "Event controllers",
    color = "#d4a72c",
    shape = "diamond",
    known = true,
    complete = true,
    notes = "Can only be activated by jump-from-below or Ground Pound.\nSwitches an event on, then when hit again; off, then when hit again; on....etc",
    grabBounds = { x = 0, y = 0, width = 16, height = 16 },
    renderBounds = { x = 0, y = 0, width = 16, height = 16 },
    sourceFiles = {
        "/obj/A_block_ncg.bin",
    },
    fields = {
        {
            key = "target_id",
            label = "Target ID",
            bitOffset = 32,
            bitWidth = 8,
        },
    },
}
