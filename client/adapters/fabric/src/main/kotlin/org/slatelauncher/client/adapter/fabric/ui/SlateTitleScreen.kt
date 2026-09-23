package org.slatelauncher.client.adapter.fabric.ui

import com.mojang.realmsclient.RealmsMainScreen
import net.minecraft.client.gui.GuiGraphics
import net.minecraft.client.gui.screens.Screen
import net.minecraft.client.gui.screens.multiplayer.JoinMultiplayerScreen
import net.minecraft.client.gui.screens.options.AccessibilityOptionsScreen
import net.minecraft.client.gui.screens.options.LanguageSelectScreen
import net.minecraft.client.gui.screens.worldselection.SelectWorldScreen
import net.minecraft.network.chat.Component

@Suppress("MagicNumber")
internal class SlateTitleScreen(private val screens: SlateScreenController) :
    Screen(Component.translatable("slate.screen.title")) {
    override fun init() {
        val layout = layout()
        var buttonY = layout.panelY + if (layout.compact) 46 else 60
        addButton(layout.contentX, buttonY, layout.contentWidth, "menu.singleplayer", true) {
            minecraft?.setScreen(SelectWorldScreen(this))
        }
        buttonY += SlateUiTheme.BUTTON_HEIGHT + SlateUiTheme.GAP
        addButton(layout.contentX, buttonY, layout.contentWidth, "menu.multiplayer") {
            minecraft?.setScreen(JoinMultiplayerScreen(this))
        }
        buttonY += SlateUiTheme.BUTTON_HEIGHT + SlateUiTheme.GAP
        addButton(layout.contentX, buttonY, layout.contentWidth, "menu.online") {
            minecraft?.setScreen(RealmsMainScreen(this))
        }
        buttonY += SlateUiTheme.BUTTON_HEIGHT + 10
        val halfWidth = (layout.contentWidth - SlateUiTheme.GAP) / 2
        addButton(layout.contentX, buttonY, halfWidth, "menu.options") {
            minecraft?.setScreen(SlateOptionsScreen(this))
        }
        addButton(
            layout.contentX + halfWidth + SlateUiTheme.GAP,
            buttonY,
            halfWidth,
            "slate.menu.control_center",
        ) {
            screens.openControlCenter(this)
        }
        buttonY += SlateUiTheme.BUTTON_HEIGHT + SlateUiTheme.GAP
        addButton(layout.contentX, buttonY, layout.contentWidth, "menu.quit") {
            minecraft?.stop()
        }
        if (!layout.compact) {
            buttonY += SlateUiTheme.BUTTON_HEIGHT + SlateUiTheme.GAP
            addButton(layout.contentX, buttonY, halfWidth, "options.language") {
                val client = minecraft ?: return@addButton
                client.setScreen(LanguageSelectScreen(this, client.options, client.languageManager))
            }
            addButton(
                layout.contentX + halfWidth + SlateUiTheme.GAP,
                buttonY,
                halfWidth,
                "options.accessibility",
            ) {
                val client = minecraft ?: return@addButton
                client.setScreen(AccessibilityOptionsScreen(this, client.options))
            }
        }
    }

    override fun render(graphics: GuiGraphics, mouseX: Int, mouseY: Int, partialTick: Float) {
        renderPanorama(graphics, partialTick)
        graphics.fill(0, 0, width, height, 0x660A100D)
        val layout = layout()
        SlateScreenGraphics.drawPanel(
            graphics,
            layout.panelX,
            layout.panelY,
            layout.panelWidth,
            layout.panelHeight,
        )
        SlateScreenGraphics.drawBrand(graphics, font, layout.contentX, layout.panelY + 14)
        if (!layout.compact) {
            graphics.drawString(
                font,
                Component.translatable("slate.title.edition"),
                layout.contentX,
                layout.panelY + 41,
                SlateUiTheme.textMuted,
                false,
            )
        }
        graphics.drawString(
            font,
            Component.translatable("slate.title.version", screens.clientVersion),
            10,
            height - 14,
            SlateUiTheme.text,
            true,
        )
        super.render(graphics, mouseX, mouseY, partialTick)
    }

    private fun layout(): TitleLayout {
        val panelWidth = minOf(330, width - 24)
        val compact = height < 300
        val panelHeight = if (compact) 208 else 260
        val panelX = (width - panelWidth) / 2
        val panelY = (height - panelHeight) / 2
        return TitleLayout(
            panelX = panelX,
            panelY = panelY,
            panelWidth = panelWidth,
            panelHeight = panelHeight,
            contentX = panelX + 16,
            contentWidth = panelWidth - 32,
            compact = compact,
        )
    }

    private fun addButton(
        x: Int,
        y: Int,
        width: Int,
        translationKey: String,
        primary: Boolean = false,
        action: () -> Unit,
    ): SlateButton = addRenderableWidget(
        SlateButton(
            x,
            y,
            width,
            Component.translatable(translationKey),
            if (primary) SlateButton.Style.PRIMARY else SlateButton.Style.SECONDARY,
            action,
        ),
    )

    private data class TitleLayout(
        val panelX: Int,
        val panelY: Int,
        val panelWidth: Int,
        val panelHeight: Int,
        val contentX: Int,
        val contentWidth: Int,
        val compact: Boolean,
    )
}
