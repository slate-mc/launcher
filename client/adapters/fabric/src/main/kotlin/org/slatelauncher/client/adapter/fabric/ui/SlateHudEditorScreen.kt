@file:Suppress("MagicNumber")

package org.slatelauncher.client.adapter.fabric.ui

import net.minecraft.client.gui.GuiGraphics
import net.minecraft.client.gui.screens.Screen
import net.minecraft.network.chat.Component
import org.slatelauncher.client.hud.HudElementId
import org.slatelauncher.client.hud.ResolvedHudElement

private const val HUD_GRID_SIZE: Int = 8

internal class SlateHudEditorScreen(
    private val previous: Screen?,
    private val screens: SlateScreenController,
) : Screen(Component.translatable("slate.screen.hud_editor")) {
    private var selectedId: HudElementId = SlateHudState.FPS
    private var dragging: Boolean = false

    override fun init() {
        installNavigation()
        installElementButtons()
        installInspectorButtons()
    }

    override fun render(graphics: GuiGraphics, mouseX: Int, mouseY: Int, partialTick: Float) {
        if (minecraft?.level == null) {
            renderPanorama(graphics, partialTick)
            graphics.fill(0, 0, width, height, 0x550A100D)
        }
        val layout = editorLayout(width, height)
        drawEditorGrid(graphics, layout)
        drawHudPreviews(
            graphics,
            font,
            screens.hudState.resolve(width, height),
            selectedId,
        )
        SlateTopBar.draw(graphics, font, layout.topBar, SlateTopBar.Tab.HUD)
        drawHudElementPanel(graphics, font, layout)
        if (layout.showInspector) {
            drawHudInspector(
                graphics,
                font,
                layout,
                hudElementName(selectedId),
                screens.hudState.resolve(width, height).find { it.id == selectedId },
            )
        }
        drawHudBottomTools(graphics, font, layout, width, height)
        super.render(graphics, mouseX, mouseY, partialTick)
    }

    override fun mouseClicked(mouseX: Double, mouseY: Double, button: Int): Boolean {
        val childHandled = super.mouseClicked(mouseX, mouseY, button)
        val element =
            if (!childHandled && button == 0) {
                findHudElementAt(screens.hudState.resolve(width, height), mouseX, mouseY)
            } else {
                null
            }
        if (element != null) {
            selectedId = element.id
            dragging = true
            rebuildWidgets()
        }
        return childHandled || element != null
    }

    override fun mouseDragged(
        mouseX: Double,
        mouseY: Double,
        button: Int,
        dragX: Double,
        dragY: Double,
    ): Boolean {
        if (dragging && button == 0) {
            screens.hudState.move(selectedId, dragX, dragY)
            return true
        }
        return super.mouseDragged(mouseX, mouseY, button, dragX, dragY)
    }

    override fun mouseReleased(mouseX: Double, mouseY: Double, button: Int): Boolean {
        if (dragging && button == 0) {
            dragging = false
            screens.hudState.snap(selectedId, HUD_GRID_SIZE)
            return true
        }
        return super.mouseReleased(mouseX, mouseY, button)
    }

    override fun onClose() {
        minecraft?.setScreen(previous)
    }

    override fun isPauseScreen(): Boolean = true

    private fun installNavigation() {
        val topBar = SlateTopBar.layout(width)
        addRenderableWidget(
            SlateButton(
                topBar.x + 84,
                topBar.y + 5,
                56,
                Component.translatable("slate.nav.mods"),
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
                SlateButton.Style.PRIMARY,
            ) {},
        )
        addRenderableWidget(
            SlateButton(
                topBar.x + topBar.width - 62,
                topBar.y + 5,
                54,
                Component.translatable("gui.done"),
                SlateButton.Style.PRIMARY,
                ::onClose,
            ),
        )
    }

    private fun installElementButtons() {
        val layout = editorLayout(width, height)
        screens.hudState.layouts().forEachIndexed { index, element ->
            addRenderableWidget(
                SlateButton(
                    layout.leftX + 10,
                    layout.contentY + 39 + index * 30,
                    layout.leftWidth - 20,
                    hudElementName(element.id),
                    if (element.id == selectedId) {
                        SlateButton.Style.PRIMARY
                    } else {
                        SlateButton.Style.SECONDARY
                    },
                ) {
                    selectedId = element.id
                    rebuildWidgets()
                },
            )
        }
    }

    private fun installInspectorButtons() {
        val layout = editorLayout(width, height)
        if (!layout.showInspector) {
            return
        }
        val centerX = layout.rightX + layout.rightWidth / 2
        val y = layout.contentY + 70
        addNudgeButton(centerX - 12, y, "↑", 0.0, -HUD_GRID_SIZE.toDouble())
        addNudgeButton(centerX - 38, y + 28, "←", -HUD_GRID_SIZE.toDouble(), 0.0)
        addNudgeButton(centerX - 12, y + 28, "↓", 0.0, HUD_GRID_SIZE.toDouble())
        addNudgeButton(centerX + 14, y + 28, "→", HUD_GRID_SIZE.toDouble(), 0.0)
        addRenderableWidget(
            SlateButton(
                layout.rightX + 10,
                layout.contentY + layout.contentHeight - 34,
                layout.rightWidth - 20,
                Component.translatable("slate.hud.undo"),
            ) {
                screens.hudState.undo()
            },
        )
    }

    private fun addNudgeButton(x: Int, y: Int, label: String, deltaX: Double, deltaY: Double) {
        addRenderableWidget(
            SlateButton(x, y, 24, Component.literal(label)) {
                screens.hudState.move(selectedId, deltaX, deltaY)
            },
        )
    }
}

private fun drawEditorGrid(graphics: GuiGraphics, layout: EditorLayout) {
    val startX = layout.leftX + layout.leftWidth + 12
    val endX = if (layout.showInspector) layout.rightX - 12 else graphics.guiWidth() - 12
    for (x in startX..endX step HUD_GRID_SIZE) {
        for (y in layout.contentY..graphics.guiHeight() - 30 step HUD_GRID_SIZE) {
            graphics.fill(x, y, x + 1, y + 1, 0x557A8A82)
        }
    }
}

private fun drawHudPreviews(
    graphics: GuiGraphics,
    font: net.minecraft.client.gui.Font,
    elements: List<ResolvedHudElement>,
    selectedId: HudElementId,
) {
    for (element in elements) {
        val x = element.x.toInt()
        val y = element.y.toInt()
        val elementWidth = element.width.toInt()
        val elementHeight = element.height.toInt()
        SlateScreenGraphics.fillRounded(
            graphics,
            x,
            y,
            elementWidth,
            elementHeight,
            SlateUiTheme.canvas,
        )
        SlateScreenGraphics.outlineRounded(
            graphics,
            x,
            y,
            elementWidth,
            elementHeight,
            if (element.id == selectedId) SlateUiTheme.jade else SlateUiTheme.border,
        )
        graphics.drawString(
            font,
            hudPreviewText(element.id),
            x + 6,
            y + 5,
            SlateUiTheme.text,
            false,
        )
    }
}

private fun drawHudElementPanel(
    graphics: GuiGraphics,
    font: net.minecraft.client.gui.Font,
    layout: EditorLayout,
) {
    SlateScreenGraphics.drawPanel(
        graphics,
        layout.leftX,
        layout.contentY,
        layout.leftWidth,
        layout.contentHeight,
    )
    graphics.drawString(
        font,
        Component.translatable("slate.hud.elements"),
        layout.leftX + 10,
        layout.contentY + 14,
        SlateUiTheme.text,
        false,
    )
}

private fun drawHudInspector(
    graphics: GuiGraphics,
    font: net.minecraft.client.gui.Font,
    layout: EditorLayout,
    selectedName: Component,
    selected: ResolvedHudElement?,
) {
    SlateScreenGraphics.drawPanel(
        graphics,
        layout.rightX,
        layout.contentY,
        layout.rightWidth,
        layout.contentHeight,
    )
    graphics.drawString(
        font,
        selectedName,
        layout.rightX + 10,
        layout.contentY + 14,
        SlateUiTheme.text,
        false,
    )
    if (selected != null) {
        graphics.drawString(
            font,
            "X ${selected.x.toInt()}  Y ${selected.y.toInt()}",
            layout.rightX + 10,
            layout.contentY + 38,
            SlateUiTheme.textMuted,
            false,
        )
    }
    graphics.drawString(
        font,
        Component.translatable("slate.hud.nudge"),
        layout.rightX + 10,
        layout.contentY + 55,
        SlateUiTheme.textMuted,
        false,
    )
}

private fun drawHudBottomTools(
    graphics: GuiGraphics,
    font: net.minecraft.client.gui.Font,
    layout: EditorLayout,
    screenWidth: Int,
    screenHeight: Int,
) {
    val toolsWidth = minOf(220, screenWidth - layout.leftWidth - 32)
    val toolsX = (screenWidth - toolsWidth) / 2
    SlateScreenGraphics.drawPanel(graphics, toolsX, screenHeight - 28, toolsWidth, 22)
    graphics.drawCenteredString(
        font,
        Component.translatable("slate.hud.grid", HUD_GRID_SIZE),
        screenWidth / 2,
        screenHeight - 21,
        SlateUiTheme.textMuted,
    )
}

private fun findHudElementAt(
    elements: List<ResolvedHudElement>,
    mouseX: Double,
    mouseY: Double,
): ResolvedHudElement? = elements.lastOrNull { element ->
    mouseX >= element.x &&
        mouseX <= element.x + element.width &&
        mouseY >= element.y &&
        mouseY <= element.y + element.height
}

private fun hudElementName(id: HudElementId): Component =
    Component.translatable("slate.hud.${id.value.substringAfter("slate.")}.name")

private fun hudPreviewText(id: HudElementId): String = when (id) {
    SlateHudState.FPS -> "144 FPS"
    SlateHudState.COORDINATES -> "XYZ 128 / 64 / -256"
    else -> id.value
}

private fun editorLayout(screenWidth: Int, screenHeight: Int): EditorLayout {
    val topBar = SlateTopBar.layout(screenWidth)
    val contentY = topBar.bottom + 8
    val contentHeight = screenHeight - contentY - 36
    val leftWidth = if (screenWidth < 560) 118 else 150
    val showInspector = screenWidth >= 560
    val rightWidth = if (showInspector) 170 else 0
    return EditorLayout(
        topBar = topBar,
        contentY = contentY,
        contentHeight = contentHeight,
        leftX = 8,
        leftWidth = leftWidth,
        rightX = screenWidth - rightWidth - 8,
        rightWidth = rightWidth,
        showInspector = showInspector,
    )
}

private data class EditorLayout(
    val topBar: SlateTopBar.Layout,
    val contentY: Int,
    val contentHeight: Int,
    val leftX: Int,
    val leftWidth: Int,
    val rightX: Int,
    val rightWidth: Int,
    val showInspector: Boolean,
)
