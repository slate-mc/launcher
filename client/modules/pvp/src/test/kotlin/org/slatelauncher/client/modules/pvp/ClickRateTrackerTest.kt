package org.slatelauncher.client.modules.pvp

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertThrows
import org.junit.jupiter.api.Test

internal class ClickRateTrackerTest {
    @Test
    fun `click rate uses a bounded rolling window`() {
        val tracker = ClickRateTracker()
        tracker.record(0)
        tracker.record(500)
        tracker.record(999)
        assertEquals(3.0, tracker.clicksPerSecond(999))
        assertEquals(2.0, tracker.clicksPerSecond(1_000))
    }

    @Test
    fun `out of order events are rejected`() {
        val tracker = ClickRateTracker()
        tracker.record(100)
        assertThrows(IllegalArgumentException::class.java) { tracker.record(99) }
    }
}
