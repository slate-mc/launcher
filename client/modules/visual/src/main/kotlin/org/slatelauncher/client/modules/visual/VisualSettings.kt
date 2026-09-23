package org.slatelauncher.client.modules.visual

public data class VisualSettings(
    public val damageTiltStrength: Double = 1.0,
    public val viewBobbingStrength: Double = 1.0,
    public val hideFireOverlay: Boolean = false,
    public val cleanScreenshotMode: Boolean = false,
) {
    init {
        require(damageTiltStrength in 0.0..1.0) { "damage tilt strength must be between 0 and 1" }
        require(viewBobbingStrength in 0.0..1.0) { "view bobbing strength must be between 0 and 1" }
    }
}
