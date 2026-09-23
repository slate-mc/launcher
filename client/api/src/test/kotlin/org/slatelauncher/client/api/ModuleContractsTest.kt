package org.slatelauncher.client.api

import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertThrows
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

internal class ModuleContractsTest {
    @Test
    fun `module identifiers reject display names and paths`() {
        assertThrows(IllegalArgumentException::class.java) { ModuleId.parse("HUD Module") }
        assertThrows(IllegalArgumentException::class.java) { ModuleId.parse("../hud") }
    }

    @Test
    fun `module compatibility is exact`() {
        val descriptor =
            ModuleDescriptor(
                id = ModuleId.parse("slate.hud"),
                version = SemanticVersion(1, 0, 0),
                capabilities = setOf(ModuleCapability.HUD),
                supportedTargets =
                    setOf(ModuleTarget("1.21.1", LoaderKind.FABRIC, "0.16.14")),
            )

        assertTrue(descriptor.supports(GameTarget("1.21.1", LoaderKind.FABRIC, "0.16.14", 21)))
        assertFalse(descriptor.supports(GameTarget("1.21.1", LoaderKind.FABRIC, "0.16.13", 21)))
        assertFalse(descriptor.supports(GameTarget("1.21.1", LoaderKind.NEOFORGE, "21.1.200", 21)))
    }
}
