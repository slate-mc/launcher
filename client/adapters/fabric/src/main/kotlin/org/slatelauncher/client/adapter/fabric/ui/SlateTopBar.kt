package org.slatelauncher.client.adapter.fabric.ui

import net.minecraft.client.gui.Font
import net.minecraft.client.gui.GuiGraphics

@Suppress("MagicNumber")
internal object SlateTopBar {
    fun layout(screenWidth: Int): Layout {
        val preferredWidth = screenWidth * 84 / 100
        val width = minOf(760, maxOf(300, preferredWidth), screenWidth - 16)
        return Layout((screenWidth - width) / 2, 10, width, 38, screenWidth < 600)
    }

    fun draw(graphics: GuiGraphics, font: Font, layout: Layout) {
        SlateScreenGraphics.drawPanel(graphics, layout.x, layout.y, layout.width, layout.height)
        SlateScreenGraphics.drawBrandLockup(graphics, font, layout.x + 10, layout.y + 8)
    }

    data class Layout(
        val x: Int,
        val y: Int,
        val width: Int,
        val height: Int,
        val compact: Boolean,
    ) {
        val bottom: Int
            get() = y + height
    }

    enum class Tab(val translationKey: String) {
        MODS("slate.nav.mods"),
        HUD("slate.nav.hud"),
        PROFILES("slate.nav.profiles"),
        SETTINGS("slate.nav.settings"),
    }
}
