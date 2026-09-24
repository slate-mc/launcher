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
        installWorldActions(layout)
        installSecondaryActions(layout)
        if (!layout.compact) installUtilities(layout)
    }

    private fun installWorldActions(layout: TitleLayout) {
        var buttonY = layout.panelY + 47
        addMenuButton(
            layout.contentX,
            buttonY,
            layout.contentWidth,
            "menu.singleplayer",
            SlateButton.Icon.WORLD,
            primary = true,
        ) {
            minecraft?.setScreen(SelectWorldScreen(this))
        }
        buttonY += SlateUiTheme.BUTTON_HEIGHT + SlateUiTheme.GAP
        addMenuButton(
            layout.contentX,
            buttonY,
            layout.contentWidth,
            "menu.multiplayer",
            SlateButton.Icon.GLOBE,
        ) {
            minecraft?.setScreen(JoinMultiplayerScreen(this))
        }
        buttonY += SlateUiTheme.BUTTON_HEIGHT + SlateUiTheme.GAP
        addMenuButton(
            layout.contentX,
            buttonY,
            layout.contentWidth,
            "menu.online",
            SlateButton.Icon.CROWN,
        ) {
            minecraft?.setScreen(RealmsMainScreen(this))
        }
    }

    private fun installSecondaryActions(layout: TitleLayout) {
        var buttonY =
            layout.panelY + 47 + (SlateUiTheme.BUTTON_HEIGHT + SlateUiTheme.GAP) * 2
        buttonY += SlateUiTheme.BUTTON_HEIGHT + 8
        val halfWidth = (layout.contentWidth - SlateUiTheme.GAP) / 2
        addMenuButton(
            layout.contentX,
            buttonY,
            halfWidth,
            "menu.options",
            SlateButton.Icon.SLIDERS,
            showChevron = false,
        ) {
            minecraft?.setScreen(SlateOptionsScreen(this))
        }
        addMenuButton(
            layout.contentX + halfWidth + SlateUiTheme.GAP,
            buttonY,
            halfWidth,
            "slate.menu.mods",
            SlateButton.Icon.SLATE,
            showChevron = false,
        ) {
            screens.openControlCenter(this)
        }
        buttonY += SlateUiTheme.BUTTON_HEIGHT + SlateUiTheme.GAP
        addMenuButton(
            layout.contentX,
            buttonY,
            layout.contentWidth,
            "menu.quit",
            SlateButton.Icon.EXIT,
            showChevron = false,
        ) {
            minecraft?.stop()
        }
    }

    private fun installUtilities(layout: TitleLayout) {
        val halfWidth = (layout.contentWidth - SlateUiTheme.GAP) / 2
        val utilityY = layout.panelY + layout.panelHeight - 27
        addRenderableWidget(
            SlateButton(
                layout.contentX,
                utilityY,
                halfWidth,
                Component.translatable("slate.title.language"),
                SlateButton.Style.SECONDARY,
                icon = SlateButton.Icon.GLOBE,
                buttonHeight = 19,
            ) {
                val client = minecraft ?: return@SlateButton
                client.setScreen(
                    LanguageSelectScreen(this, client.options, client.languageManager),
                )
            },
        )
        addRenderableWidget(
            SlateButton(
                layout.contentX + halfWidth + SlateUiTheme.GAP,
                utilityY,
                halfWidth,
                Component.translatable("slate.title.accessibility"),
                SlateButton.Style.SECONDARY,
                icon = SlateButton.Icon.SLIDERS,
                buttonHeight = 19,
            ) {
                val client = minecraft ?: return@SlateButton
                client.setScreen(AccessibilityOptionsScreen(this, client.options))
            },
        )
    }

    override fun renderBackground(
        graphics: GuiGraphics,
        mouseX: Int,
        mouseY: Int,
        partialTick: Float,
    ) {
        renderPanorama(graphics, partialTick)
        renderBlurredBackground(partialTick)
        graphics.fill(0, 0, width, height, 0x4D07100C)
        val layout = layout()
        SlateScreenGraphics.drawPanel(
            graphics,
            layout.panelX,
            layout.panelY,
            layout.panelWidth,
            layout.panelHeight,
        )
        SlateScreenGraphics.drawBrandLockup(graphics, font, layout.contentX, layout.panelY + 12)
        graphics.drawString(
            font,
            SlateTypography.mono(Component.translatable("slate.title.edition")),
            layout.contentX,
            layout.panelY + 34,
            SlateUiTheme.textMuted,
            false,
        )
        val disclaimer =
            SlateTypography.mono(Component.translatable("slate.title.disclaimer"))
        graphics.drawString(
            font,
            SlateTypography.mono(
                Component.translatable("slate.title.version", screens.clientVersion),
            ),
            10,
            height - 14,
            SlateUiTheme.textSecondary,
            false,
        )
        graphics.drawString(
            font,
            disclaimer,
            width - font.width(disclaimer) - 10,
            height - 14,
            SlateUiTheme.textMuted,
            false,
        )
    }

    private fun layout(): TitleLayout {
        val compact = height < 300
        val widthPercent = if (compact) 40 else 52
        val panelWidth = minOf(252, maxOf(176, width * widthPercent / 100), width - 24)
        val panelHeight = minOf(if (compact) 182 else 218, height - 16)
        val panelX = (width - panelWidth) / 2
        val panelY = (height - panelHeight) / 2
        return TitleLayout(
            panelX = panelX,
            panelY = panelY,
            panelWidth = panelWidth,
            panelHeight = panelHeight,
            contentX = panelX + 12,
            contentWidth = panelWidth - 24,
            compact = compact,
        )
    }

    private fun addMenuButton(
        x: Int,
        y: Int,
        width: Int,
        translationKey: String,
        icon: SlateButton.Icon,
        primary: Boolean = false,
        showChevron: Boolean = true,
        action: () -> Unit,
    ): SlateButton = addRenderableWidget(
        SlateButton(
            x,
            y,
            width,
            Component.translatable(translationKey),
            if (primary) SlateButton.Style.PRIMARY else SlateButton.Style.SECONDARY,
            icon = icon,
            alignment = SlateButton.Alignment.LEFT,
            showChevron = showChevron,
            action = action,
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
