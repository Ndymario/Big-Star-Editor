-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 82,
    name = "Platform Block (expandable block)",
    category = "Platforms and ropes",
    color = "#4f86c6",
    shape = "box",
    known = true,
    complete = true,
    grabBounds = { x = 0, y = 0, width = 16, height = 16 },
    renderBounds = { x = 0, y = 0, width = 16, height = 16 },
    sourceFiles = {
        "/obj/A_block_ncg.bin ([SOMETHING]_block_[SOMETHING].[SOMETHING])",
    },
    fields = {
        {
            key = "turn_used_at_4th_hit",
            label = "Turn used at 4th hit",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Turns into used blocks. Unused. All values above 1 seem to produce same result as 1.",
            checkboxValue = 1,
        },
    },
}
