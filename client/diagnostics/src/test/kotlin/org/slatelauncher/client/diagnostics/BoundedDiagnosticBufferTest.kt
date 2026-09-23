package org.slatelauncher.client.diagnostics

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Test
import org.slatelauncher.client.api.ModuleId

internal class BoundedDiagnosticBufferTest {
    private val moduleId = ModuleId.parse("slate.performance")

    @Test
    fun `buffer is bounded and removes secret values`() {
        val buffer =
            BoundedDiagnosticBuffer(capacity = 2, privateValues = setOf("C:\\Users\\Player"))
        buffer.append(record(1, DiagnosticSeverity.INFO, "path C:\\Users\\Player"))
        buffer.append(record(2, DiagnosticSeverity.WARNING, "Bearer visible-token"))
        buffer.append(record(3, DiagnosticSeverity.ERROR, "failed"))

        val records = buffer.snapshot()
        assertEquals(2, records.size)
        assertFalse(records.joinToString().contains("visible-token"))
        assertEquals(1, buffer.health(moduleId).warningCount)
        assertEquals(1, buffer.health(moduleId).errorCount)
    }

    private fun record(
        timestamp: Long,
        severity: DiagnosticSeverity,
        message: String,
    ): DiagnosticRecord = DiagnosticRecord(timestamp, moduleId, severity, "module.event", message)
}
