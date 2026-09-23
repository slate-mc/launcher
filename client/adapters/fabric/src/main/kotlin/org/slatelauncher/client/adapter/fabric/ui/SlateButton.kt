package org.slatelauncher.client.adapter.fabric.ui

import net.minecraft.client.Minecraft
import net.minecraft.client.gui.GuiGraphics
import net.minecraft.client.gui.components.AbstractButton
import net.minecraft.client.gui.narration.NarrationElementOutput
import net.minecraft.network.chat.Component

@Suppress("MagicNumber")
internal class SlateButton(
    x: Int,
    y: Int,
    width: Int,
    message: Component,
    private val style: Style = Style.SECONDARY,
    private val action: () -> Unit,
) : AbstractButton(x, y, width, SlateUiTheme.BUTTON_HEIGHT, message) {
    override fun onPress() {
        action()
    }

    override fun renderWidget(
        graphics: GuiGraphics,
        mouseX: Int,
        mouseY: Int,
        partialTick: Float,
    ) {
        val emphasized = isHoveredOrFocused
        val background =
            when {
                !active -> SlateUiTheme.disabled
                style == Style.PRIMARY -> SlateUiTheme.jade
                style == Style.DANGER && emphasized -> SlateUiTheme.danger
                emphasized -> SlateUiTheme.surfaceRaised
                else -> SlateUiTheme.surface
            }
        val foreground =
            when {
                style == Style.PRIMARY -> SlateUiTheme.jadeInk
                style == Style.DANGER && emphasized -> SlateUiTheme.jadeInk
                else -> SlateUiTheme.text
            }
        SlateScreenGraphics.fillRounded(graphics, x, y, width, height, background)
        SlateScreenGraphics.outlineRounded(
            graphics,
            x,
            y,
            width,
            height,
            if (emphasized) SlateUiTheme.jade else SlateUiTheme.border,
        )
        graphics.drawCenteredString(
            Minecraft.getInstance().font,
            message,
            x + width / 2,
            y + (height - 8) / 2,
            foreground,
        )
    }

    override fun updateWidgetNarration(output: NarrationElementOutput) {
        defaultButtonNarrationText(output)
    }

    internal enum class Style {
        PRIMARY,
        SECONDARY,
        DANGER,
    }
}
