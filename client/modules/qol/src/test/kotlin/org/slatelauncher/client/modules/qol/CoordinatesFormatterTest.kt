package org.slatelauncher.client.modules.qol

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

internal class CoordinatesFormatterTest {
    @Test
    fun `coordinates use a locale-independent representation`() {
        assertEquals(
            "12.3   64.0  -8.5  overworld",
            CoordinatesFormatter.format(WorldPosition(12.25, 64.0, -8.5, "overworld")),
        )
    }
}
