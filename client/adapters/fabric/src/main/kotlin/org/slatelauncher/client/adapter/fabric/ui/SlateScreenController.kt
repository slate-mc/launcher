package org.slatelauncher.client.adapter.fabric.ui

import com.mojang.blaze3d.platform.InputConstants
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper
import net.minecraft.client.KeyMapping
import net.minecraft.client.Minecraft
import net.minecraft.client.gui.screens.PauseScreen
import net.minecraft.client.gui.screens.Screen
import net.minecraft.client.gui.screens.TitleScreen
import org.slatelauncher.client.runtime.ModuleSupervisor

internal class SlateScreenController(
    private val supervisor: ModuleSupervisor,
    val clientVersion: String,
) {
    internal val hudState: SlateHudState = SlateHudState()
    private val controlCenterKey =
        KeyBindingHelper.registerKeyBinding(
            KeyMapping(
                "key.slate-client.control_center",
                InputConstants.Type.KEYSYM,
                InputConstants.KEY_RSHIFT,
                "key.categories.slate-client",
            ),
        )

    fun register() {
        ClientTickEvents.END_CLIENT_TICK.register(::onClientTick)
        SlateHudOverlay(supervisor, hudState).register()
    }

    fun openControlCenter(previous: Screen?) {
        Minecraft.getInstance().setScreen(SlateControlCenterScreen(previous, supervisor, this))
    }

    fun openHudEditor(previous: Screen?) {
        Minecraft.getInstance().setScreen(SlateHudEditorScreen(previous, this))
    }

    fun titleScreen(): Screen = SlateTitleScreen(this)

    private fun onClientTick(client: Minecraft) {
        while (controlCenterKey.consumeClick()) {
            val current = client.screen
            when (current) {
                is SlateControlCenterScreen -> current.onClose()
                is SlateHudEditorScreen -> current.onClose()
                else -> openControlCenter(current)
            }
        }
        when (val current = client.screen) {
            is TitleScreen -> client.setScreen(titleScreen())

            is PauseScreen -> {
                if (current.showsPauseMenu()) {
                    client.setScreen(SlatePauseScreen(this))
                }
            }
        }
    }
}
