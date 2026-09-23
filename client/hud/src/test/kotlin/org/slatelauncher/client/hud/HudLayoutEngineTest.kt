package org.slatelauncher.client.hud

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

internal class HudLayoutEngineTest {
    @Test
    fun `layout remains inside the safe viewport`() {
        val layout =
            HudElementLayout(
                id = HudElementId.parse("slate.fps"),
                anchor = HudAnchor.BOTTOM_RIGHT,
                offsetX = 500.0,
                offsetY = 500.0,
                width = 80.0,
                height = 20.0,
                scale = 1.25,
            )
        val resolved =
            HudLayoutEngine.resolve(
                layout,
                Viewport(1_920.0, 1_080.0, Insets(12.0, 12.0, 12.0, 48.0)),
            )

        assertEquals(1_808.0, resolved.x)
        assertEquals(1_007.0, resolved.y)
        assertTrue(resolved.x + resolved.width <= 1_908.0)
        assertTrue(resolved.y + resolved.height <= 1_032.0)
    }

    @Test
    fun `keyboard moves are recoverable`() {
        val original =
            HudElementLayout(
                HudElementId.parse("slate.coords"),
                HudAnchor.TOP_LEFT,
                0.0,
                0.0,
                100.0,
                20.0,
            )
        val editor = HudEditorSession(listOf(original))
        editor.move(original.id, 1.0, 0.0)

        assertEquals(8.0, editor.snapshot().single().offsetX)
        assertTrue(editor.undo())
        assertEquals(original, editor.snapshot().single())
    }
}
