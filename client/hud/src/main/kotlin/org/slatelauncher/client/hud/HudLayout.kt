package org.slatelauncher.client.hud

private const val MINIMUM_HUD_SCALE: Double = 0.5
private const val MAXIMUM_HUD_SCALE: Double = 3.0

@JvmInline
public value class HudElementId private constructor(public val value: String) :
    Comparable<HudElementId> {
    override fun compareTo(other: HudElementId): Int = value.compareTo(other.value)

    override fun toString(): String = value

    public companion object {
        private val VALID_ID = Regex("[a-z][a-z0-9._-]{2,63}")

        public fun parse(value: String): HudElementId {
            require(VALID_ID.matches(value)) { "HUD element ID is invalid" }
            return HudElementId(value)
        }
    }
}

public enum class HudAnchor {
    TOP_LEFT,
    TOP_CENTER,
    TOP_RIGHT,
    CENTER_LEFT,
    CENTER,
    CENTER_RIGHT,
    BOTTOM_LEFT,
    BOTTOM_CENTER,
    BOTTOM_RIGHT,
}

public enum class ScreenVisibility {
    GAMEPLAY,
    CHAT,
    INVENTORY,
    DEBUG,
}

public data class Insets(
    public val left: Double = 0.0,
    public val top: Double = 0.0,
    public val right: Double = 0.0,
    public val bottom: Double = 0.0,
) {
    init {
        require(listOf(left, top, right, bottom).all { it >= 0.0 && it.isFinite() }) {
            "safe-area insets must be finite and non-negative"
        }
    }
}

public data class Viewport(
    public val width: Double,
    public val height: Double,
    public val safeArea: Insets = Insets(),
) {
    init {
        require(width > 0.0 && height > 0.0 && width.isFinite() && height.isFinite()) {
            "viewport dimensions must be finite and positive"
        }
        require(safeArea.left + safeArea.right < width) { "horizontal safe area is too large" }
        require(safeArea.top + safeArea.bottom < height) { "vertical safe area is too large" }
    }
}

public data class HudElementLayout(
    public val id: HudElementId,
    public val anchor: HudAnchor,
    public val offsetX: Double,
    public val offsetY: Double,
    public val width: Double,
    public val height: Double,
    public val scale: Double = 1.0,
    public val visibility: Set<ScreenVisibility> = setOf(ScreenVisibility.GAMEPLAY),
) {
    init {
        require(offsetX.isFinite() && offsetY.isFinite()) { "HUD offsets must be finite" }
        require(width > 0.0 && height > 0.0 && width.isFinite() && height.isFinite()) {
            "HUD dimensions must be finite and positive"
        }
        require(scale in MINIMUM_HUD_SCALE..MAXIMUM_HUD_SCALE) {
            "HUD scale must be between $MINIMUM_HUD_SCALE and $MAXIMUM_HUD_SCALE"
        }
        require(visibility.isNotEmpty()) { "HUD element must be visible on at least one screen" }
    }
}

public data class ResolvedHudElement(
    public val id: HudElementId,
    public val x: Double,
    public val y: Double,
    public val width: Double,
    public val height: Double,
)
