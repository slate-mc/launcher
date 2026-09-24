package org.slatelauncher.client.adapter.fabric.ui

import net.minecraft.client.gui.Font
import net.minecraft.client.gui.GuiGraphics
import net.minecraft.client.gui.components.EditBox
import net.minecraft.network.chat.Component

/** A native edit box with Slate's visual and type treatment. */
internal class SlateTextField(
    font: Font,
    visualX: Int,
    y: Int,
    visualWidth: Int,
    height: Int,
    hint: Component,
) : EditBox(
    font,
    visualX + HORIZONTAL_PADDING,
    y,
    visualWidth - HORIZONTAL_PADDING * 2,
    height,
    hint,
) {
    private val fieldX = visualX
    private val fieldWidth = visualWidth

    init {
        setBordered(false)
        setTextColor(SlateUiTheme.text)
        setTextColorUneditable(SlateUiTheme.textMuted)
        setHint(SlateTypography.ui(hint))
        setFormatter { text, _ ->
            SlateTypography.ui(Component.literal(text)).visualOrderText
        }
    }

    override fun renderWidget(
        graphics: GuiGraphics,
        mouseX: Int,
        mouseY: Int,
        partialTick: Float,
    ) {
        SlateScreenGraphics.fillRounded(
            graphics,
            fieldX,
            y,
            fieldWidth,
            height,
            SlateUiTheme.surface,
        )
        SlateScreenGraphics.outlineRounded(
            graphics,
            fieldX,
            y,
            fieldWidth,
            height,
            if (isFocused) SlateUiTheme.jade else SlateUiTheme.border,
        )
        super.renderWidget(graphics, mouseX, mouseY, partialTick)
    }

    private companion object {
        const val HORIZONTAL_PADDING = 7
    }
}
