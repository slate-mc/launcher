package org.slatelauncher.client.adapter.fabric.ui

import net.minecraft.client.gui.Font
import net.minecraft.client.gui.GuiGraphics
import net.minecraft.network.chat.Component
import net.minecraft.resources.ResourceLocation

@Suppress("MagicNumber")
internal object SlateScreenGraphics {
    private val symbolTexture =
        ResourceLocation.fromNamespaceAndPath(
            "slate-client",
            "textures/gui/slate-symbol-paper.png",
        )

    fun drawBackdrop(graphics: GuiGraphics, width: Int, height: Int) {
        graphics.fill(0, 0, width, height, SlateUiTheme.canvas)
        graphics.fill(0, 0, 5, height, SlateUiTheme.jade)
        graphics.fill(5, height - 3, width, height, SlateUiTheme.surfaceRaised)
    }

    fun drawBrandLockup(graphics: GuiGraphics, font: Font, x: Int, y: Int) {
        drawBrandSymbol(graphics, x, y + 1)
        graphics.drawString(
            font,
            SlateTypography.brand(Component.literal("slate")),
            x + 27,
            y + 3,
            SlateUiTheme.text,
            false,
        )
    }

    fun drawScaledString(
        graphics: GuiGraphics,
        font: Font,
        text: Component,
        x: Int,
        y: Int,
        scale: Float,
        color: Int,
    ) {
        graphics.pose().pushPose()
        graphics.pose().translate(x.toFloat(), y.toFloat(), 0.0f)
        graphics.pose().scale(scale, scale, 1.0f)
        graphics.drawString(font, SlateTypography.ui(text), 0, 0, color, false)
        graphics.pose().popPose()
    }

    private fun drawBrandSymbol(graphics: GuiGraphics, x: Int, y: Int) {
        graphics.blit(
            symbolTexture,
            x,
            y,
            22,
            19,
            0.0f,
            0.0f,
            176,
            152,
            176,
            152,
        )
    }

    fun drawPanel(graphics: GuiGraphics, x: Int, y: Int, width: Int, height: Int) {
        fillRounded(graphics, x + 3, y + 4, width, height, SlateUiTheme.SHADOW)
        fillRounded(graphics, x, y, width, height, SlateUiTheme.surface)
        outlineRounded(graphics, x, y, width, height, SlateUiTheme.border)
    }

    fun fillRounded(graphics: GuiGraphics, x: Int, y: Int, width: Int, height: Int, color: Int) {
        graphics.fill(x + 3, y, x + width - 3, y + height, color)
        graphics.fill(x + 1, y + 1, x + width - 1, y + height - 1, color)
        graphics.fill(x, y + 3, x + width, y + height - 3, color)
    }

    fun outlineRounded(
        graphics: GuiGraphics,
        x: Int,
        y: Int,
        width: Int,
        height: Int,
        color: Int,
    ) {
        graphics.hLine(x + 3, x + width - 4, y, color)
        graphics.hLine(x + 3, x + width - 4, y + height - 1, color)
        graphics.vLine(x, y + 3, y + height - 4, color)
        graphics.vLine(x + width - 1, y + 3, y + height - 4, color)
        graphics.fill(x + 1, y + 1, x + 3, y + 2, color)
        graphics.fill(x + width - 3, y + 1, x + width - 1, y + 2, color)
        graphics.fill(x + 1, y + height - 2, x + 3, y + height - 1, color)
        graphics.fill(x + width - 3, y + height - 2, x + width - 1, y + height - 1, color)
    }
}
