package org.slatelauncher.client.adapter.fabric.ui

import net.minecraft.client.gui.Font
import net.minecraft.client.gui.GuiGraphics

@Suppress("MagicNumber")
internal object SlateScreenGraphics {
    fun drawBackdrop(graphics: GuiGraphics, width: Int, height: Int) {
        graphics.fill(0, 0, width, height, SlateUiTheme.canvas)
        graphics.fill(0, 0, 5, height, SlateUiTheme.jade)
        graphics.fill(5, height - 3, width, height, SlateUiTheme.surfaceRaised)
    }

    fun drawBrand(graphics: GuiGraphics, font: Font, x: Int, y: Int) {
        graphics.fill(x, y, x + 22, y + 22, SlateUiTheme.jade)
        graphics.fill(x + 6, y + 5, x + 16, y + 8, SlateUiTheme.jadeInk)
        graphics.fill(x + 6, y + 8, x + 10, y + 13, SlateUiTheme.jadeInk)
        graphics.fill(x + 10, y + 12, x + 16, y + 15, SlateUiTheme.jadeInk)
        graphics.fill(x + 12, y + 15, x + 16, y + 18, SlateUiTheme.jadeInk)
        graphics.drawString(font, "SLATE", x + 30, y + 2, SlateUiTheme.text, false)
        graphics.drawString(font, "CLIENT", x + 30, y + 13, SlateUiTheme.textMuted, false)
    }

    fun drawPanel(graphics: GuiGraphics, x: Int, y: Int, width: Int, height: Int) {
        fillRounded(graphics, x, y, width, height, SlateUiTheme.surface)
        outlineRounded(graphics, x, y, width, height, SlateUiTheme.border)
    }

    fun fillRounded(graphics: GuiGraphics, x: Int, y: Int, width: Int, height: Int, color: Int) {
        graphics.fill(x + 2, y, x + width - 2, y + height, color)
        graphics.fill(x, y + 2, x + width, y + height - 2, color)
    }

    fun outlineRounded(
        graphics: GuiGraphics,
        x: Int,
        y: Int,
        width: Int,
        height: Int,
        color: Int,
    ) {
        graphics.hLine(x + 2, x + width - 3, y, color)
        graphics.hLine(x + 2, x + width - 3, y + height - 1, color)
        graphics.vLine(x, y + 2, y + height - 3, color)
        graphics.vLine(x + width - 1, y + 2, y + height - 3, color)
        graphics.fill(x + 1, y + 1, x + 2, y + 2, color)
        graphics.fill(x + width - 2, y + 1, x + width - 1, y + 2, color)
        graphics.fill(x + 1, y + height - 2, x + 2, y + height - 1, color)
        graphics.fill(
            x + width - 2,
            y + height - 2,
            x + width - 1,
            y + height - 1,
            color,
        )
    }
}
