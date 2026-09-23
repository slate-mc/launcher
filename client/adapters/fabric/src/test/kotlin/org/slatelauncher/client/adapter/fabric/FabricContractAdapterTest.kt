package org.slatelauncher.client.adapter.fabric

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertThrows
import org.junit.jupiter.api.Test
import org.slatelauncher.client.api.AdapterStatus
import org.slatelauncher.client.api.GameTarget
import org.slatelauncher.client.api.LoaderKind

internal class FabricContractAdapterTest {
    @Test
    fun `adapter refuses another loader`() {
        val adapter = FabricContractAdapter("1.21.1", "0.16.14")
        assertThrows(IllegalArgumentException::class.java) {
            adapter.handshake(
                GameTarget("1.21.1", LoaderKind.NEOFORGE, "21.1.200", 21),
                emptyList(),
            )
        }
    }

    @Test
    fun `contract-only status cannot be mistaken for game integration`() {
        val target = GameTarget("1.21.1", LoaderKind.FABRIC, "0.16.14", 21)
        val handshake = FabricContractAdapter("1.21.1", "0.16.14").handshake(target, emptyList())
        assertEquals(AdapterStatus.CONTRACT_ONLY, handshake.adapterStatus)
    }
}
