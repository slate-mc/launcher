package org.slatelauncher.client.adapter.fabric.ui

import java.util.Locale
import net.fabricmc.fabric.api.client.rendering.v1.HudRenderCallback
import net.minecraft.client.Minecraft
import net.minecraft.client.gui.GuiGraphics
import net.minecraft.network.chat.Component
import org.slatelauncher.client.api.ModuleId
import org.slatelauncher.client.api.ModuleState
import org.slatelauncher.client.hud.ResolvedHudElement
import org.slatelauncher.client.runtime.ModuleSupervisor

@Suppress("MagicNumber")
internal class SlateHudOverlay(
    private val supervisor: ModuleSupervisor,
    private val state: SlateHudState,
) {
    fun register() {
        HudRenderCallback.EVENT.register { graphics, _ -> render(graphics) }
    }

    private fun render(graphics: GuiGraphics) {
        val client = Minecraft.getInstance()
        if (client.level == null || client.options.hideGui) {
            return
        }
        for (element in state.resolve(graphics.guiWidth(), graphics.guiHeight())) {
            when (element.id) {
                SlateHudState.FPS -> {
                    if (isActive(PERFORMANCE_MODULE)) {
                        drawBadge(graphics, element, "${client.fps} FPS")
                    }
                }

                SlateHudState.COORDINATES -> {
                    val player = client.player
                    if (player != null && isActive(QOL_MODULE)) {
                        val coordinates =
                            String.format(
                                Locale.ROOT,
                                "XYZ %.1f / %.1f / %.1f",
                                player.x,
                                player.y,
                                player.z,
                            )
                        drawBadge(graphics, element, coordinates)
                    }
                }
            }
        }
    }

    private fun drawBadge(graphics: GuiGraphics, element: ResolvedHudElement, text: String) {
        val font = Minecraft.getInstance().font
        val x = element.x.toInt()
        val y = element.y.toInt()
        val width = maxOf(element.width.toInt(), font.width(text) + 14)
        val height = element.height.toInt()
        graphics.fill(x, y, x + width, y + height, SlateUiTheme.canvas)
        graphics.fill(x, y, x + 3, y + height, SlateUiTheme.jade)
        graphics.renderOutline(x, y, width, height, SlateUiTheme.border)
        graphics.drawString(
            font,
            SlateTypography.mono(Component.literal(text)),
            x + 8,
            y + 5,
            SlateUiTheme.text,
            false,
        )
    }

    private fun isActive(moduleId: ModuleId): Boolean =
        supervisor.snapshot()[moduleId] == ModuleState.ACTIVE

    private companion object {
        val PERFORMANCE_MODULE: ModuleId = ModuleId.parse("slate.performance")
        val QOL_MODULE: ModuleId = ModuleId.parse("slate.qol")
    }
}
