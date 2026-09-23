package org.slatelauncher.client.diagnostics

import org.slatelauncher.client.api.ModuleId

public enum class DiagnosticSeverity {
    DEBUG,
    INFO,
    WARNING,
    ERROR,
}

public data class DiagnosticRecord(
    public val timestampEpochMillis: Long,
    public val moduleId: ModuleId,
    public val severity: DiagnosticSeverity,
    public val code: String,
    public val message: String,
) {
    init {
        require(timestampEpochMillis >= 0) { "diagnostic timestamp cannot be negative" }
        require(CODE_PATTERN.matches(code)) { "diagnostic code is invalid" }
    }

    private companion object {
        val CODE_PATTERN: Regex = Regex("[a-z][a-z0-9_.-]{2,63}")
    }
}

public data class ModuleHealthSnapshot(
    public val moduleId: ModuleId,
    public val warningCount: Int,
    public val errorCount: Int,
    public val latestTimestampEpochMillis: Long?,
)
