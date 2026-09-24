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
        var buttonY = layout.actionsY
        addAction(
            layout,
            buttonY,
            "menu.returnToGame",
            SlateButton.Icon.PLAY,
            SlateButton.Style.PRIMARY,
        ) {
            onClose()
        }
        buttonY += layout.rowHeight + layout.rowGap + 6

        val client = minecraft
        val player = client?.player
        addAction(layout, buttonY, "gui.advancements", SlateButton.Icon.TROPHY) {
            val advancements = player?.connection?.advancements ?: return@addAction
            minecraft?.setScreen(AdvancementsScreen(advancements, this))
        }.active = player != null
        buttonY += layout.rowHeight + layout.rowGap
        addAction(layout, buttonY, "gui.stats", SlateButton.Icon.CHART) {
            val stats = player?.stats ?: return@addAction
            minecraft?.setScreen(StatsScreen(this, stats))
        }.active = player != null
        buttonY += layout.rowHeight + layout.rowGap
        addAction(layout, buttonY, "menu.options", SlateButton.Icon.SLIDERS) {
            minecraft?.setScreen(SlateOptionsScreen(this))
        }
        buttonY += layout.rowHeight + layout.rowGap
        addAction(layout, buttonY, "menu.shareToLan", SlateButton.Icon.NETWORK) {
            minecraft?.setScreen(ShareToLanScreen(this))
        }.active = client?.hasSingleplayerServer() == true
        buttonY += layout.rowHeight + layout.rowGap + 8

        addAction(layout, buttonY, "slate.menu.control_center", SlateButton.Icon.SLATE) {
            screens.openControlCenter(this)
        }
        buttonY += layout.rowHeight + layout.rowGap
        addAction(layout, buttonY, "slate.menu.edit_hud", SlateButton.Icon.SLIDERS) {
            screens.openHudEditor(this)
        }
        buttonY += layout.rowHeight + layout.rowGap + 8
        addAction(layout, buttonY, "menu.returnToMenu", SlateButton.Icon.EXIT) {
            minecraft?.disconnect(screens.titleScreen())
        }
    }

    override fun renderBackground(
        graphics: GuiGraphics,
        mouseX: Int,
        mouseY: Int,
        partialTick: Float,
    ) {
        renderBlurredBackground(partialTick)
        renderTransparentBackground(graphics)
        val layout = layout()
        graphics.fill(0, 0, layout.railWidth, height, SlateUiTheme.sidebar)
        graphics.vLine(layout.railWidth - 1, 0, height, SlateUiTheme.border)
        SlateScreenGraphics.drawBrandLockup(graphics, font, layout.contentX, 9)
        graphics.drawString(
            font,
            SlateTypography.mono(
                Component.literal(
                    if (minecraft?.isSingleplayer == true) "Singleplayer" else "Multiplayer",
                ),
            ),
            layout.contentX + 74,
            16,
            SlateUiTheme.textMuted,
            false,
        )
        SlateScreenGraphics.drawScaledString(
            graphics,
            font,
            Component.translatable("slate.pause.heading.short"),
            layout.contentX,
            35,
            if (height < 310) 1.2f else 1.5f,
            SlateUiTheme.text,
        )
        graphics.drawString(
            font,
            SlateTypography.mono(Component.literal("1.21.1 / Fabric")),
            layout.contentX,
            height - 14,
            SlateUiTheme.textMuted,
            false,
        )
    }

    override fun onClose() {
        minecraft?.setScreen(null)
    }

    override fun isPauseScreen(): Boolean = true

    private fun layout(): PauseLayout {
        val railWidth = minOf(210, maxOf(150, width * 31 / 100))
        val compact = height < 300
        return PauseLayout(
            railWidth = railWidth,
            contentX = 14,
            contentWidth = railWidth - 28,
            actionsY = if (compact) 52 else 70,
            rowHeight = if (compact) 18 else SlateUiTheme.BUTTON_HEIGHT,
            rowGap = if (compact) 1 else 3,
        )
    }

    private fun addAction(
        layout: PauseLayout,
        y: Int,
        translationKey: String,
        icon: SlateButton.Icon,
        style: SlateButton.Style = SlateButton.Style.GHOST,
        action: () -> Unit = {},
    ): SlateButton = addRenderableWidget(
        SlateButton(
            layout.contentX,
            y,
            layout.contentWidth,
            Component.translatable(translationKey),
            style,
            icon = icon,
            alignment = SlateButton.Alignment.LEFT,
            showChevron = style != SlateButton.Style.PRIMARY,
            buttonHeight = layout.rowHeight,
            action = action,
        ),
    )

    private data class PauseLayout(
        val railWidth: Int,
        val contentX: Int,
        val contentWidth: Int,
        val actionsY: Int,
        val rowHeight: Int,
        val rowGap: Int,
    )
}
