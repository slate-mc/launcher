package org.slatelauncher.client.adapter.fabric.ui

import net.minecraft.client.gui.GuiGraphics
import net.minecraft.client.gui.screens.Screen
import net.minecraft.network.chat.Component

@Suppress("MagicNumber")
internal class SlateSectionScreen(
    private val previous: Screen,
    private val tab: SlateTopBar.Tab,
    private val screens: SlateScreenController,
) : Screen(Component.translatable(tab.translationKey)) {
    override fun init() {
        val topBar = SlateTopBar.layout(width)
        addRenderableWidget(
            SlateButton(
                topBar.x + 84,
                topBar.y + 5,
                56,
                Component.translatable("slate.nav.mods"),
                SlateButton.Style.TAB,
            ) {
                screens.openControlCenter(previous)
            },
        )
        addRenderableWidget(
            SlateButton(
                topBar.x + 146,
                topBar.y + 5,
                72,
                Component.translatable("slate.nav.hud"),
                SlateButton.Style.TAB,
            ) {
                screens.openHudEditor(previous)
            },
        )
        addRenderableWidget(
            SlateButton(
                topBar.x + topBar.width - 62,
                topBar.y + 5,
                54,
                Component.translatable("gui.done"),
                SlateButton.Style.PRIMARY,
                action = ::onClose,
            ),
        )
    }

    override fun renderBackground(
        graphics: GuiGraphics,
        mouseX: Int,
        mouseY: Int,
        partialTick: Float,
    ) {
        if (minecraft?.level != null) {
            renderBlurredBackground(partialTick)
            renderTransparentBackground(graphics)
        } else {
            renderPanorama(graphics, partialTick)
            renderBlurredBackground(partialTick)
            graphics.fill(0, 0, width, height, 0x660A100D)
        }
        val topBar = SlateTopBar.layout(width)
        SlateTopBar.draw(graphics, font, topBar)
        val panelY = topBar.bottom + 8
        val panelHeight = height - panelY - 8
        SlateScreenGraphics.drawPanel(graphics, topBar.x, panelY, topBar.width, panelHeight)
        graphics.drawString(
            font,
            SlateTypography.ui(title),
            topBar.x + 18,
            panelY + 18,
            SlateUiTheme.text,
            false,
        )
        graphics.drawWordWrap(
            font,
            SlateTypography.ui(
                Component.translatable("slate.section.${tab.name.lowercase()}.description"),
            ),
            topBar.x + 18,
            panelY + 40,
            topBar.width - 36,
            SlateUiTheme.textMuted,
        )
    }

    override fun onClose() {
        minecraft?.setScreen(previous)
    }
}
