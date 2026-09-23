package org.slatelauncher.client.adapter.fabric.ui

import net.minecraft.client.gui.GuiGraphics
import net.minecraft.client.gui.screens.Screen
import net.minecraft.client.gui.screens.options.AccessibilityOptionsScreen
import net.minecraft.client.gui.screens.options.ChatOptionsScreen
import net.minecraft.client.gui.screens.options.LanguageSelectScreen
import net.minecraft.client.gui.screens.options.OnlineOptionsScreen
import net.minecraft.client.gui.screens.options.SkinCustomizationScreen
import net.minecraft.client.gui.screens.options.SoundOptionsScreen
import net.minecraft.client.gui.screens.options.VideoSettingsScreen
import net.minecraft.client.gui.screens.options.controls.ControlsScreen
import net.minecraft.network.chat.Component

@Suppress("MagicNumber")
internal class SlateOptionsScreen(private val previous: Screen) :
    Screen(Component.translatable("options.title")) {
    override fun init() {
        val layout = layout()
        val halfWidth = (layout.contentWidth - SlateUiTheme.GAP) / 2
        val client = minecraft ?: return
        val entries =
            listOf(
                "options.video" to
                    { client.setScreen(VideoSettingsScreen(this, client, client.options)) },
                "options.sounds" to { client.setScreen(SoundOptionsScreen(this, client.options)) },
                "options.controls" to { client.setScreen(ControlsScreen(this, client.options)) },
                "options.language" to {
                    client.setScreen(
                        LanguageSelectScreen(this, client.options, client.languageManager),
                    )
                },
                "options.chat" to
                    { client.setScreen(ChatOptionsScreen(this, client.options)) },
                "options.skinCustomisation" to {
                    client.setScreen(SkinCustomizationScreen(this, client.options))
                },
                "options.accessibility" to {
                    client.setScreen(AccessibilityOptionsScreen(this, client.options))
                },
                "options.online" to {
                    client.setScreen(OnlineOptionsScreen(this, client.options))
                },
            )
        entries.forEachIndexed { index, (translationKey, action) ->
            val column = index % 2
            val row = index / 2
            addRenderableWidget(
                SlateButton(
                    layout.contentX + column * (halfWidth + SlateUiTheme.GAP),
                    layout.panelY + 56 + row * (SlateUiTheme.BUTTON_HEIGHT + SlateUiTheme.GAP),
                    halfWidth,
                    Component.translatable(translationKey),
                    action = action,
                ),
            )
        }
        addRenderableWidget(
            SlateButton(
                layout.contentX,
                layout.panelY + layout.panelHeight - 40,
                layout.contentWidth,
                Component.translatable("gui.done"),
                SlateButton.Style.PRIMARY,
                ::onClose,
            ),
        )
    }

    override fun render(graphics: GuiGraphics, mouseX: Int, mouseY: Int, partialTick: Float) {
        if (minecraft?.level == null) {
            renderPanorama(graphics, partialTick)
            graphics.fill(0, 0, width, height, 0x660A100D)
        } else {
            renderTransparentBackground(graphics)
        }
        val layout = layout()
        SlateScreenGraphics.drawPanel(
            graphics,
            layout.panelX,
            layout.panelY,
            layout.panelWidth,
            layout.panelHeight,
        )
        SlateScreenGraphics.drawBrand(graphics, font, layout.contentX, layout.panelY + 14)
        graphics.drawString(
            font,
            title,
            layout.panelX + layout.panelWidth - 16 - font.width(title),
            layout.panelY + 20,
            SlateUiTheme.text,
            false,
        )
        super.render(graphics, mouseX, mouseY, partialTick)
    }

    override fun onClose() {
        minecraft?.setScreen(previous)
    }

    private fun layout(): OptionsLayout {
        val panelWidth = minOf(430, width - 24)
        val panelHeight = minOf(236, height - 16)
        val panelX = (width - panelWidth) / 2
        val panelY = (height - panelHeight) / 2
        return OptionsLayout(
            panelX = panelX,
            panelY = panelY,
            panelWidth = panelWidth,
            panelHeight = panelHeight,
            contentX = panelX + 16,
            contentWidth = panelWidth - 32,
        )
    }

    private data class OptionsLayout(
        val panelX: Int,
        val panelY: Int,
        val panelWidth: Int,
        val panelHeight: Int,
        val contentX: Int,
        val contentWidth: Int,
    )
}
