package org.slatelauncher.client.modules.performance

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

internal class FrameTimeTrackerTest {
    @Test
    fun `summary is deterministic and bounded`() {
        val tracker = FrameTimeTracker(capacity = 30)
        repeat(35) { tracker.record((it + 1).toDouble()) }

        val summary = requireNotNull(tracker.summary())
        assertEquals(30, summary.sampleCount)
        assertEquals(20.5, summary.averageMillis)
        assertEquals(34.0, summary.percentile95Millis)
        assertEquals(35.0, summary.worstMillis)
    }
}
