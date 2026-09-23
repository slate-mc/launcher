package org.slatelauncher.client.modules.accessibility

private const val MINIMUM_TEXT_SCALE: Double = 0.75
private const val MAXIMUM_TEXT_SCALE: Double = 2.0

public data class AccessibilitySettings(
    public val reduceHudMotion: Boolean = false,
    public val highContrastHud: Boolean = false,
    public val textScale: Double = 1.0,
    public val holdInsteadOfToggle: Boolean = false,
) {
    init {
        require(textScale in MINIMUM_TEXT_SCALE..MAXIMUM_TEXT_SCALE) {
            "accessibility text scale must be between $MINIMUM_TEXT_SCALE and $MAXIMUM_TEXT_SCALE"
        }
    }
}
