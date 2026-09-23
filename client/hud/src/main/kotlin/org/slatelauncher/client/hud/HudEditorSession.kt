package org.slatelauncher.client.hud

private const val DEFAULT_HISTORY_LIMIT: Int = 50
private const val MINIMUM_HISTORY_LIMIT: Int = 1
private const val MAXIMUM_HISTORY_LIMIT: Int = 200
private const val DEFAULT_MOVE_MULTIPLIER: Double = 8.0

public class HudEditorSession(
    initial: Collection<HudElementLayout>,
    private val historyLimit: Int = DEFAULT_HISTORY_LIMIT,
) {
    private var layouts: Map<HudElementId, HudElementLayout> = initial.associateUnique()
    private val history: ArrayDeque<Map<HudElementId, HudElementLayout>> = ArrayDeque()

    init {
        require(historyLimit in MINIMUM_HISTORY_LIMIT..MAXIMUM_HISTORY_LIMIT) {
            "history limit must be between $MINIMUM_HISTORY_LIMIT and $MAXIMUM_HISTORY_LIMIT"
        }
    }

    public fun snapshot(): List<HudElementLayout> = layouts.values.sortedBy { it.id }

    public fun update(layout: HudElementLayout) {
        require(layout.id in layouts) { "HUD element ${layout.id} is not in this layout" }
        remember()
        layouts = layouts + (layout.id to layout)
    }

    public fun move(id: HudElementId, deltaX: Double, deltaY: Double, precise: Boolean = false) {
        val layout = requireNotNull(layouts[id]) { "HUD element $id is not in this layout" }
        val multiplier = if (precise) 1.0 else DEFAULT_MOVE_MULTIPLIER
        update(
            layout.copy(
                offsetX = layout.offsetX + deltaX * multiplier,
                offsetY = layout.offsetY + deltaY * multiplier,
            ),
        )
    }

    public fun undo(): Boolean {
        val previous = history.removeLastOrNull() ?: return false
        layouts = previous
        return true
    }

    private fun remember() {
        if (history.size == historyLimit) {
            history.removeFirst()
        }
        history.addLast(layouts)
    }
}

private fun Collection<HudElementLayout>.associateUnique(): Map<HudElementId, HudElementLayout> {
    val result = linkedMapOf<HudElementId, HudElementLayout>()
    for (layout in this) {
        require(result.put(layout.id, layout) == null) { "duplicate HUD element ${layout.id}" }
    }
    return result
}
