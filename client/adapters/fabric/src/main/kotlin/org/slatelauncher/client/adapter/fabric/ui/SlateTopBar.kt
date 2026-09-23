package org.slatelauncher.client.adapter.fabric.ui

import net.minecraft.client.gui.Font
import net.minecraft.client.gui.GuiGraphics
import net.minecraft.network.chat.Component

@Suppress("MagicNumber")
internal object SlateTopBar {
    fun layout(screenWidth: Int): Layout {
        val width = minOf(900, screenWidth - 16)
        return Layout((screenWidth - width) / 2, 8, width, 34, screenWidth < 560)
    }

    fun draw(graphics: GuiGraphics, font: Font, layout: Layout, active: Tab) {
        SlateScreenGraphics.drawPanel(graphics, layout.x, layout.y, layout.width, layout.height)
        SlateScreenGraphics.drawBrand(graphics, font, layout.x + 10, layout.y + 6)
        val activeLabel = Component.translatable(active.translationKey)
        graphics.drawString(
            font,
            activeLabel,
            layout.x + layout.width - 72 - font.width(activeLabel),
            layout.y + 13,
            SlateUiTheme.textMuted,
            false,
        )
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
