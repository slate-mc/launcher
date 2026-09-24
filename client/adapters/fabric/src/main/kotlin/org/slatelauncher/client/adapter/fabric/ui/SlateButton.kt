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
    private val icon: Icon? = null,
    private val alignment: Alignment = Alignment.CENTER,
    private val showChevron: Boolean = false,
    buttonHeight: Int = SlateUiTheme.BUTTON_HEIGHT,
    private val action: () -> Unit,
) : AbstractButton(x, y, width, buttonHeight, SlateTypography.ui(message)) {
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
        val foreground = foregroundColor(emphasized)
        SlateScreenGraphics.fillRounded(graphics, x, y, width, height, backgroundColor(emphasized))
        drawFrame(graphics, emphasized)
        drawContent(graphics, foreground)
    }

    private fun backgroundColor(emphasized: Boolean): Int = when {
        !active -> SlateUiTheme.disabled
        style == Style.PRIMARY -> if (emphasized) 0xFFA2E2C0.toInt() else SlateUiTheme.jade
        style == Style.DANGER && emphasized -> SlateUiTheme.danger
        style == Style.GHOST && emphasized -> SlateUiTheme.hover
        style == Style.GHOST -> 0x00101413
        style == Style.TAB_ACTIVE -> SlateUiTheme.surfaceRaised
        style == Style.TAB -> if (emphasized) SlateUiTheme.hover else 0x00101413
        emphasized -> SlateUiTheme.hover
        else -> SlateUiTheme.surface
    }

    private fun foregroundColor(emphasized: Boolean): Int = when {
        style == Style.PRIMARY -> SlateUiTheme.jadeInk
        style == Style.DANGER && emphasized -> SlateUiTheme.jadeInk
        else -> SlateUiTheme.text
    }

    private fun drawFrame(graphics: GuiGraphics, emphasized: Boolean) {
        if (style != Style.GHOST && style != Style.TAB && style != Style.TAB_ACTIVE) {
            SlateScreenGraphics.outlineRounded(
                graphics,
                x,
                y,
                width,
                height,
                if (emphasized) SlateUiTheme.jade else SlateUiTheme.border,
            )
        }
        if (style == Style.TAB_ACTIVE) {
            graphics.fill(x + 8, y + height - 2, x + width - 8, y + height, SlateUiTheme.jade)
        }
    }

    private fun drawContent(graphics: GuiGraphics, foreground: Int) {
        val font = Minecraft.getInstance().font
        val iconOffset = if (icon == null) 0 else 17
        val textX =
            when (alignment) {
                Alignment.CENTER -> x + (width - font.width(message) + iconOffset) / 2
                Alignment.LEFT -> x + 12 + iconOffset
            }
        icon?.draw(graphics, textX - 17, y + (height - 10) / 2, foreground)
        graphics.drawString(font, message, textX, y + (height - 9) / 2, foreground, false)
        if (showChevron) {
            val chevronX = x + width - 13
            val chevronY = y + height / 2
            graphics.fill(chevronX, chevronY - 3, chevronX + 1, chevronY - 1, foreground)
            graphics.fill(chevronX + 1, chevronY - 2, chevronX + 2, chevronY + 2, foreground)
            graphics.fill(chevronX, chevronY + 1, chevronX + 1, chevronY + 3, foreground)
        }
    }

    override fun updateWidgetNarration(output: NarrationElementOutput) {
        defaultButtonNarrationText(output)
    }

    internal enum class Style {
        PRIMARY,
        SECONDARY,
        DANGER,
        GHOST,
        TAB,
        TAB_ACTIVE,
    }

    internal enum class Alignment {
        LEFT,
        CENTER,
    }

    internal enum class Icon {
        PLAY,
        WORLD,
        GLOBE,
        CROWN,
        SLIDERS,
        SLATE,
        EXIT,
        TROPHY,
        CHART,
        NETWORK,
        ;

        fun draw(graphics: GuiGraphics, x: Int, y: Int, color: Int) {
            when (this) {
                PLAY -> {
                    graphics.fill(x + 2, y + 1, x + 4, y + 9, color)
                    graphics.fill(x + 4, y + 3, x + 6, y + 8, color)
                    graphics.fill(x + 6, y + 4, x + 8, y + 7, color)
                }

                WORLD -> {
                    SlateScreenGraphics.outlineRounded(graphics, x + 1, y + 1, 9, 9, color)
                    graphics.hLine(x + 2, x + 8, y + 5, color)
                    graphics.vLine(x + 5, y + 2, y + 8, color)
                }

                GLOBE -> {
                    SlateScreenGraphics.outlineRounded(graphics, x + 1, y + 1, 9, 9, color)
                    graphics.hLine(x + 1, x + 9, y + 5, color)
                    graphics.vLine(x + 5, y + 1, y + 9, color)
                }

                CROWN -> {
                    graphics.fill(x + 1, y + 6, x + 10, y + 9, color)
                    graphics.fill(x + 2, y + 3, x + 4, y + 7, color)
                    graphics.fill(x + 5, y + 1, x + 7, y + 7, color)
                    graphics.fill(x + 8, y + 3, x + 10, y + 7, color)
                }

                SLIDERS -> {
                    graphics.hLine(x + 1, x + 10, y + 2, color)
                    graphics.hLine(x + 1, x + 10, y + 5, color)
                    graphics.hLine(x + 1, x + 10, y + 8, color)
                    graphics.fill(x + 3, y + 1, x + 5, y + 4, color)
                    graphics.fill(x + 7, y + 4, x + 9, y + 7, color)
                    graphics.fill(x + 4, y + 7, x + 6, y + 10, color)
                }

                SLATE -> {
                    graphics.fill(x + 1, y + 1, x + 9, y + 4, color)
                    graphics.fill(x + 3, y + 6, x + 11, y + 9, color)
                }

                EXIT -> {
                    SlateScreenGraphics.outlineRounded(graphics, x + 1, y + 1, 7, 9, color)
                    graphics.hLine(x + 5, x + 11, y + 5, color)
                    graphics.fill(x + 9, y + 3, x + 10, y + 8, color)
                }

                TROPHY -> {
                    graphics.fill(x + 3, y + 1, x + 8, y + 6, color)
                    graphics.fill(x + 5, y + 6, x + 7, y + 9, color)
                    graphics.hLine(x + 3, x + 9, y + 9, color)
                }

                CHART -> {
                    graphics.fill(x + 1, y + 6, x + 3, y + 10, color)
                    graphics.fill(x + 4, y + 3, x + 6, y + 10, color)
                    graphics.fill(x + 7, y + 1, x + 9, y + 10, color)
                }

                NETWORK -> {
                    graphics.fill(x + 4, y + 1, x + 7, y + 4, color)
                    graphics.fill(x + 1, y + 7, x + 4, y + 10, color)
                    graphics.fill(x + 7, y + 7, x + 10, y + 10, color)
                    graphics.hLine(x + 2, x + 8, y + 6, color)
                    graphics.vLine(x + 5, y + 3, y + 7, color)
                }
            }
        }
    }
}
