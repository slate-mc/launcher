@file:Suppress("MagicNumber")

package org.slatelauncher.client.adapter.fabric.ui

import net.minecraft.client.gui.GuiGraphics
import net.minecraft.client.gui.screens.Screen
import net.minecraft.network.chat.Component
import org.slatelauncher.client.api.ModuleDescriptor
import org.slatelauncher.client.api.ModuleId
import org.slatelauncher.client.api.ModuleState
import org.slatelauncher.client.runtime.ModuleSupervisor

internal class SlateControlCenterScreen(
    private val previous: Screen?,
    private val supervisor: ModuleSupervisor,
    private val screens: SlateScreenController,
) : Screen(Component.translatable("slate.screen.control_center")) {
    private val moduleButtons = mutableListOf<SlateButton>()
    private var searchQuery: String = ""
    private var page: Int = 0
    private var selectedId: ModuleId? = supervisor.descriptors().firstOrNull()?.id

    override fun init() {
        installNavigation()
        installSearch()
        rebuildModuleButtons()
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
        val layout = controlLayout(width, height)
        SlateTopBar.draw(graphics, font, layout.topBar)
        SlateScreenGraphics.drawPanel(
            graphics,
            layout.browserX,
            layout.contentY,
            layout.browserWidth,
            layout.contentHeight,
        )
        graphics.drawString(
            font,
            SlateTypography.ui(Component.translatable("slate.mods.heading")),
            layout.browserX + 12,
            layout.contentY + 12,
            SlateUiTheme.text,
            false,
        )
        graphics.drawString(
            font,
            SlateTypography.ui(
                Component.translatable(
                    "slate.mods.count",
                    filterModules(supervisor.descriptors(), searchQuery).size,
                ),
            ),
            layout.browserX + layout.browserWidth - 48,
            layout.contentY + 12,
            SlateUiTheme.textMuted,
            false,
        )
        if (layout.wide) {
            drawDetails(graphics, layout)
        }
    }

    override fun onClose() {
        minecraft?.setScreen(previous)
    }

    override fun isPauseScreen(): Boolean = previous?.isPauseScreen ?: minecraft?.level != null

    private fun installNavigation() {
        val topBar = controlLayout(width, height).topBar
        var x = topBar.x + 92
        addNavButton(x, topBar.y + 5, "slate.nav.mods", true) {}
        x += 62
        addNavButton(x, topBar.y + 5, "slate.nav.hud") { screens.openHudEditor(previous) }
        if (!topBar.compact) {
            x += 78
            addNavButton(x, topBar.y + 5, "slate.nav.profiles") {
                minecraft?.setScreen(
                    SlateSectionScreen(this, SlateTopBar.Tab.PROFILES, screens),
                )
            }
            x += 78
            addNavButton(x, topBar.y + 5, "slate.nav.settings") {
                minecraft?.setScreen(
                    SlateSectionScreen(this, SlateTopBar.Tab.SETTINGS, screens),
                )
            }
        }
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

    private fun installSearch() {
        val layout = controlLayout(width, height)
        val search =
            SlateTextField(
                font,
                layout.browserX + 12,
                layout.contentY + 28,
                layout.browserWidth - 24,
                20,
                Component.translatable("slate.search.modules"),
            )
        search.setValue(searchQuery)
        search.setResponder { value ->
            if (value != searchQuery) {
                searchQuery = value
                page = 0
                rebuildModuleButtons()
            }
        }
        addRenderableWidget(search)
    }

    private fun rebuildModuleButtons() {
        moduleButtons.forEach(::removeWidget)
        moduleButtons.clear()
        val layout = controlLayout(width, height)
        val modules = filterModules(supervisor.descriptors(), searchQuery)
        val pageSize = maxOf(1, (layout.contentHeight - 92) / 30)
        val pageCount = maxOf(1, (modules.size + pageSize - 1) / pageSize)
        page = page.coerceIn(0, pageCount - 1)
        modules.drop(page * pageSize).take(pageSize).forEachIndexed { index, descriptor ->
            val rowY = layout.contentY + 54 + index * 30
            val active = supervisor.snapshot()[descriptor.id] == ModuleState.ACTIVE
            val name = moduleDisplayName(descriptor)
            val label =
                if (layout.wide) {
                    name
                } else {
                    Component.translatable(
                        "slate.module.row",
                        name,
                        Component.translatable(
                            if (active) "slate.module.enabled" else "slate.module.disabled",
                        ),
                    )
                }
            val button =
                SlateButton(
                    layout.browserX + 12,
                    rowY,
                    layout.browserWidth - 24,
                    label,
                    if (descriptor.id == selectedId || (!layout.wide && active)) {
                        SlateButton.Style.PRIMARY
                    } else {
                        SlateButton.Style.SECONDARY
                    },
                ) {
                    if (layout.wide) {
                        selectedId = descriptor.id
                    } else {
                        supervisor.setEnabled(descriptor.id, !active)
                    }
                    rebuildModuleButtons()
                }
            moduleButtons += addRenderableWidget(button)
        }
        if (layout.wide) {
            installDetailToggle(layout)
        }
        if (pageCount > 1) {
            installPagination(layout, pageCount)
        }
    }

    private fun installDetailToggle(layout: ControlLayout) {
        val descriptor = supervisor.descriptors().find { it.id == selectedId } ?: return
        val active = supervisor.snapshot()[descriptor.id] == ModuleState.ACTIVE
        val button =
            SlateButton(
                layout.detailX + 16,
                layout.contentY + layout.contentHeight - 40,
                layout.detailWidth - 32,
                Component.translatable(
                    if (active) "slate.module.disable" else "slate.module.enable",
                ),
                if (active) SlateButton.Style.SECONDARY else SlateButton.Style.PRIMARY,
            ) {
                supervisor.setEnabled(descriptor.id, !active)
                rebuildModuleButtons()
            }
        moduleButtons += addRenderableWidget(button)
    }

    private fun installPagination(layout: ControlLayout, pageCount: Int) {
        val buttonWidth = (layout.browserWidth - 30) / 2
        val y = layout.contentY + layout.contentHeight - 34
        val previousButton =
            SlateButton(
                layout.browserX + 12,
                y,
                buttonWidth,
                Component.translatable("spectatorMenu.previous_page"),
            ) {
                page--
                rebuildModuleButtons()
            }
        previousButton.active = page > 0
        moduleButtons += addRenderableWidget(previousButton)
        val nextButton =
            SlateButton(
                layout.browserX + 18 + buttonWidth,
                y,
                buttonWidth,
                Component.translatable("spectatorMenu.next_page"),
            ) {
                page++
                rebuildModuleButtons()
            }
        nextButton.active = page + 1 < pageCount
        moduleButtons += addRenderableWidget(nextButton)
    }

    private fun drawDetails(graphics: GuiGraphics, layout: ControlLayout) {
        SlateScreenGraphics.drawPanel(
            graphics,
            layout.detailX,
            layout.contentY,
            layout.detailWidth,
            layout.contentHeight,
        )
        val descriptor = supervisor.descriptors().find { it.id == selectedId } ?: return
        val active = supervisor.snapshot()[descriptor.id] == ModuleState.ACTIVE
        graphics.drawString(
            font,
            SlateTypography.ui(moduleDisplayName(descriptor)),
            layout.detailX + 16,
            layout.contentY + 16,
            SlateUiTheme.text,
            false,
        )
        graphics.drawString(
            font,
            SlateTypography.ui(
                Component.translatable(
                    if (active) "slate.module.enabled" else "slate.module.disabled",
                ),
            ),
            layout.detailX + layout.detailWidth - 42,
            layout.contentY + 16,
            if (active) SlateUiTheme.jade else SlateUiTheme.textMuted,
            false,
        )
        graphics.drawWordWrap(
            font,
            SlateTypography.ui(
                Component.translatable(
                    "slate.module.${descriptor.id.value.substringAfter("slate.")}.description",
                ),
            ),
            layout.detailX + 16,
            layout.contentY + 39,
            layout.detailWidth - 32,
            SlateUiTheme.textMuted,
        )
        graphics.drawString(
            font,
            SlateTypography.mono(
                Component.translatable("slate.module.version", descriptor.version.toString()),
            ),
            layout.detailX + 16,
            layout.contentY + 84,
            SlateUiTheme.textMuted,
            false,
        )
    }

    private fun addNavButton(
        x: Int,
        y: Int,
        translationKey: String,
        active: Boolean = false,
        action: () -> Unit,
    ) {
        addRenderableWidget(
            SlateButton(
                x,
                y,
                if (translationKey == "slate.nav.hud") 72 else 56,
                Component.translatable(translationKey),
                if (active) SlateButton.Style.TAB_ACTIVE else SlateButton.Style.TAB,
                action = action,
            ),
        )
    }
}

private fun filterModules(
    descriptors: List<ModuleDescriptor>,
    query: String,
): List<ModuleDescriptor> {
    val normalized = query.trim().lowercase()
    return descriptors.filter { descriptor ->
        normalized.isEmpty() ||
            descriptor.id.value.contains(normalized) ||
            moduleDisplayName(descriptor).string.lowercase().contains(normalized)
    }
}

private fun moduleDisplayName(descriptor: ModuleDescriptor): Component = Component.translatable(
    "slate.module.${descriptor.id.value.substringAfter("slate.")}.name",
)

private fun controlLayout(screenWidth: Int, screenHeight: Int): ControlLayout {
    val workspaceHeight = minOf(420, maxOf(230, screenHeight * 82 / 100), screenHeight - 16)
    val topBar =
        SlateTopBar.layout(screenWidth).copy(y = (screenHeight - workspaceHeight) / 2)
    val contentY = topBar.bottom + 8
    val contentHeight = workspaceHeight - topBar.height - 8
    val wide = topBar.width >= 560
    val browserWidth = if (wide) minOf(240, topBar.width / 3) else topBar.width
    val detailX = topBar.x + browserWidth + 8
    return ControlLayout(
        topBar = topBar,
        contentY = contentY,
        contentHeight = contentHeight,
        browserX = topBar.x,
        browserWidth = browserWidth,
        detailX = detailX,
        detailWidth = topBar.x + topBar.width - detailX,
        wide = wide,
    )
}

private data class ControlLayout(
    val topBar: SlateTopBar.Layout,
    val contentY: Int,
    val contentHeight: Int,
    val browserX: Int,
    val browserWidth: Int,
    val detailX: Int,
    val detailWidth: Int,
    val wide: Boolean,
)
