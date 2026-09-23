package org.slatelauncher.client.adapter.fabric.ui

import org.slatelauncher.client.hud.HudAnchor
import org.slatelauncher.client.hud.HudEditorSession
import org.slatelauncher.client.hud.HudElementId
import org.slatelauncher.client.hud.HudElementLayout
import org.slatelauncher.client.hud.HudLayoutEngine
import org.slatelauncher.client.hud.ResolvedHudElement
import org.slatelauncher.client.hud.Viewport

@Suppress("MagicNumber")
internal class SlateHudState {
    private val editor =
        HudEditorSession(
            listOf(
                HudElementLayout(
                    id = FPS,
                    anchor = HudAnchor.TOP_LEFT,
                    offsetX = 8.0,
                    offsetY = 52.0,
                    width = 56.0,
                    height = 18.0,
                ),
                HudElementLayout(
                    id = COORDINATES,
                    anchor = HudAnchor.TOP_LEFT,
                    offsetX = 8.0,
                    offsetY = 76.0,
                    width = 142.0,
                    height = 18.0,
                ),
            ),
        )

    fun layouts(): List<HudElementLayout> = editor.snapshot()

    fun resolve(width: Int, height: Int): List<ResolvedHudElement> {
        val viewport = Viewport(width.toDouble(), height.toDouble())
        return layouts().map { HudLayoutEngine.resolve(it, viewport) }
    }

    fun move(id: HudElementId, deltaX: Double, deltaY: Double) {
        editor.move(id, deltaX, deltaY, precise = true)
    }

    fun snap(id: HudElementId, gridSize: Int) {
        val layout = requireNotNull(layouts().find { it.id == id })
        editor.update(HudLayoutEngine.snap(layout, gridSize))
    }

    fun undo(): Boolean = editor.undo()

    companion object {
        val FPS: HudElementId = HudElementId.parse("slate.fps")
        val COORDINATES: HudElementId = HudElementId.parse("slate.coordinates")
    }
}
