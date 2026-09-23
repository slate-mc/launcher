package org.slatelauncher.client.hud

import kotlin.math.round

private const val MINIMUM_GRID_SIZE: Int = 1
private const val MAXIMUM_GRID_SIZE: Int = 64

public object HudLayoutEngine {
    public fun resolve(layout: HudElementLayout, viewport: Viewport): ResolvedHudElement {
        val width = layout.width * layout.scale
        val height = layout.height * layout.scale
        val safeLeft = viewport.safeArea.left
        val safeTop = viewport.safeArea.top
        val safeRight = viewport.width - viewport.safeArea.right
        val safeBottom = viewport.height - viewport.safeArea.bottom
        require(width <= safeRight - safeLeft && height <= safeBottom - safeTop) {
            "HUD element is larger than the safe viewport"
        }

        val horizontal =
            when (layout.anchor) {
                HudAnchor.TOP_LEFT,
                HudAnchor.CENTER_LEFT,
                HudAnchor.BOTTOM_LEFT,
                -> safeLeft

                HudAnchor.TOP_CENTER,
                HudAnchor.CENTER,
                HudAnchor.BOTTOM_CENTER,
                -> safeLeft + (safeRight - safeLeft - width) / 2.0

                HudAnchor.TOP_RIGHT,
                HudAnchor.CENTER_RIGHT,
                HudAnchor.BOTTOM_RIGHT,
                -> safeRight - width
            }
        val vertical =
            when (layout.anchor) {
                HudAnchor.TOP_LEFT,
                HudAnchor.TOP_CENTER,
                HudAnchor.TOP_RIGHT,
                -> safeTop

                HudAnchor.CENTER_LEFT,
                HudAnchor.CENTER,
                HudAnchor.CENTER_RIGHT,
                -> safeTop + (safeBottom - safeTop - height) / 2.0

                HudAnchor.BOTTOM_LEFT,
                HudAnchor.BOTTOM_CENTER,
                HudAnchor.BOTTOM_RIGHT,
                -> safeBottom - height
            }

        return ResolvedHudElement(
            id = layout.id,
            x = (horizontal + layout.offsetX).coerceIn(safeLeft, safeRight - width),
            y = (vertical + layout.offsetY).coerceIn(safeTop, safeBottom - height),
            width = width,
            height = height,
        )
    }

    public fun snap(layout: HudElementLayout, gridSize: Int): HudElementLayout {
        require(gridSize in MINIMUM_GRID_SIZE..MAXIMUM_GRID_SIZE) {
            "grid size must be between $MINIMUM_GRID_SIZE and $MAXIMUM_GRID_SIZE"
        }
        return layout.copy(
            offsetX = round(layout.offsetX / gridSize) * gridSize,
            offsetY = round(layout.offsetY / gridSize) * gridSize,
        )
    }
}
