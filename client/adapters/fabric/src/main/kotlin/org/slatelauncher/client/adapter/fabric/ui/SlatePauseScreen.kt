package org.slatelauncher.client.adapter.fabric.ui

import net.minecraft.client.gui.GuiGraphics
import net.minecraft.client.gui.screens.Screen
import net.minecraft.client.gui.screens.ShareToLanScreen
import net.minecraft.client.gui.screens.achievement.StatsScreen
import net.minecraft.client.gui.screens.advancements.AdvancementsScreen
import net.minecraft.network.chat.Component

@Suppress("MagicNumber")
internal class SlatePauseScreen(private val screens: SlateScreenController) :
    Screen(Component.translatable("menu.game")) {
    override fun init() {
        val layout = layout()
        val halfWidth = (layout.contentWidth - SlateUiTheme.GAP) / 2
        var buttonY = layout.panelY + 61
        addButton(layout.contentX, buttonY, layout.contentWidth, "menu.returnToGame", true) {
            onClose()
        }
        buttonY += SlateUiTheme.BUTTON_HEIGHT + SlateUiTheme.GAP
        val client = minecraft
        val player = client?.player
        addButton(layout.contentX, buttonY, halfWidth, "gui.advancements") {
            val advancements = player?.connection?.advancements ?: return@addButton
            minecraft?.setScreen(AdvancementsScreen(advancements, this))
        }.active = player != null
        addButton(
            layout.contentX + halfWidth + SlateUiTheme.GAP,
            buttonY,
            halfWidth,
            "gui.stats",
        ) {
            val stats = player?.stats ?: return@addButton
            minecraft?.setScreen(StatsScreen(this, stats))
        }.active = player != null
        buttonY += SlateUiTheme.BUTTON_HEIGHT + SlateUiTheme.GAP
        addButton(layout.contentX, buttonY, halfWidth, "menu.options") {
            minecraft?.setScreen(SlateOptionsScreen(this))
        }
        addButton(
            layout.contentX + halfWidth + SlateUiTheme.GAP,
            buttonY,
            halfWidth,
            "menu.shareToLan",
        ) {
            minecraft?.setScreen(ShareToLanScreen(this))
        }.active = client?.hasSingleplayerServer() == true
        buttonY += SlateUiTheme.BUTTON_HEIGHT + SlateUiTheme.GAP
        addButton(layout.contentX, buttonY, halfWidth, "slate.menu.control_center") {
            screens.openControlCenter(this)
        }
        addButton(
            layout.contentX + halfWidth + SlateUiTheme.GAP,
            buttonY,
            halfWidth,
            "slate.menu.edit_hud",
        ) {
            screens.openHudEditor(this)
        }
        buttonY += SlateUiTheme.BUTTON_HEIGHT + 10
        addButton(layout.contentX, buttonY, layout.contentWidth, "menu.returnToMenu") {
            minecraft?.disconnect(screens.titleScreen())
        }
    }

    override fun render(graphics: GuiGraphics, mouseX: Int, mouseY: Int, partialTick: Float) {
        renderTransparentBackground(graphics)
        val layout = layout()
        SlateScreenGraphics.drawPanel(
            graphics,
            layout.panelX,
            layout.panelY,
            layout.panelWidth,
            layout.panelHeight,
        )
        SlateScreenGraphics.drawBrand(graphics, font, layout.contentX, layout.panelY + 13)
        graphics.drawString(
            font,
            Component.translatable("slate.pause.heading"),
            layout.contentX,
            layout.panelY + 40,
            SlateUiTheme.text,
            false,
        )
        graphics.drawString(
            font,
            if (minecraft?.isSingleplayer == true) "Singleplayer" else "Multiplayer",
            layout.panelX + layout.panelWidth - 80,
            layout.panelY + 40,
            SlateUiTheme.textMuted,
            false,
        )
        super.render(graphics, mouseX, mouseY, partialTick)
    }

    override fun onClose() {
        minecraft?.setScreen(null)
    }

    override fun isPauseScreen(): Boolean = true

    private fun layout(): PauseLayout {
        val panelWidth = minOf(340, width - 24)
        val panelHeight = minOf(232, height - 16)
        val panelX = (width - panelWidth) / 2
        val panelY = (height - panelHeight) / 2
        return PauseLayout(
            panelX = panelX,
            panelY = panelY,
            panelWidth = panelWidth,
            panelHeight = panelHeight,
            contentX = panelX + 16,
            contentWidth = panelWidth - 32,
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

    private data class PauseLayout(
        val panelX: Int,
        val panelY: Int,
        val panelWidth: Int,
        val panelHeight: Int,
        val contentX: Int,
        val contentWidth: Int,
    )
}
