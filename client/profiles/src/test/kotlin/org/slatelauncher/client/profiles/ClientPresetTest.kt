package org.slatelauncher.client.profiles

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test
import org.slatelauncher.client.api.ModuleId

internal class ClientPresetTest {
    @Test
    fun `PvP is one preset rather than the whole client`() {
        val presets = StandardClientPresets.defaults()
        assertEquals(PresetKind.entries.size, presets.size)
        assertTrue(
            presets.single {
                it.kind == PresetKind.PVP
            }.enabledModules.contains(ModuleId.parse("slate.pvp")),
        )
        assertTrue(
            presets.single {
                it.kind == PresetKind.SURVIVAL
            }.enabledModules.contains(ModuleId.parse("slate.qol")),
        )
    }
}
