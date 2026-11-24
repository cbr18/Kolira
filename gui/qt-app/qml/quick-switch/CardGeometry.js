.pragma library

// Card outlines as SVG path strings. One geometry drives the mask, the
// border, the glow and the hit area, so a new shape is one function.
function outline(shape, w, h, radius, skew) {
    var r = Math.max(0, Math.min(radius, Math.min(w, h) / 2))
    if (shape === "parallelogram") {
        var dx = w * skew
        return "M " + dx + ",0 L " + w + ",0 L " + (w - dx) + "," + h + " L 0," + h + " z"
    }
    if (shape === "rounded") {
        return "M " + r + ",0 L " + (w - r) + ",0 "
            + "Q " + w + ",0 " + w + "," + r + " "
            + "L " + w + "," + (h - r) + " "
            + "Q " + w + "," + h + " " + (w - r) + "," + h + " "
            + "L " + r + "," + h + " "
            + "Q 0," + h + " 0," + (h - r) + " "
            + "L 0," + r + " "
            + "Q 0,0 " + r + ",0 z"
    }
    return "M 0,0 L " + w + ",0 L " + w + "," + h + " L 0," + h + " z"
}

// Width of a card relative to its height. One box for every shape keeps the
// carousel's snapping exact; the parallelogram leaves air inside its box.
function widthFactor(shape) {
    return 0.74
}

// Horizontal gap between two cards, as a fraction of card height. The
// parallelogram is inset by skew/2 on each visible side, so its gap is
// reduced by exactly that much: neighbours then sit as close as the
// rectangular ones instead of leaving empty corners between them.
function spacingFactor(shape) {
    var base = 0.07
    if (shape !== "parallelogram") return base
    return base - 0.18 * widthFactor(shape)
}