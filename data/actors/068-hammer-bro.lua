-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 68,
    name = "Hammer Bro.",
    category = "Enemies - Overground",
    color = "#b5651d",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "Make sure to put 2 levels of ground at the location of the Hammer Bro, or he will glitch and fall in to the ground.\n\nDoes not have:\n- Any (or at least not any working-) nybbles/values that change the jumping AI, like there are in NSMBW.",
    grabBounds = { x = 0, y = -18, width = 18, height = 34 },
    renderBounds = { x = 0, y = -18, width = 18, height = 34 },
    sourceFiles = {
        "/enemy/hammer_bros.nsbmd",
        "/enemy/I_hanmmer.nsbmd",
    },
    model = readModel("hammer_bros.nsbmd"),
}
