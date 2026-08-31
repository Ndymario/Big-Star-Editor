-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 188,
    name = "Bump from below platform (Mario Bros. style platform)",
    category = "Platforms and ropes",
    color = "#4f86c6",
    shape = "box",
    known = true,
    complete = true,
    grabBounds = { x = -6, y = 0, width = 12, height = 10 },
    renderBounds = { x = -6, y = 0, width = 12, height = 10 },
    sourceFiles = {
        "/enemy/I_m_yuka.nsbtx ([SOMETHING]_[SOMETHING]_floor)",
        "/enemy/I_m_yuka_hidari.nsbtx ([SOMETHING]_[SOMETHING]_floor_left)",
        "/enemy/I_m_yuka_migi.nsbtx ([SOMETHING]_[SOMETHING]_floor_right)",
    },
    fields = {
        {
            key = "length",
            label = "Length",
            bitOffset = 0,
            bitWidth = 4,
        },
    },
    model = readModel("I_m_yuka.nsbtx"),
}
