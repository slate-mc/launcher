package org.slatelauncher.client.diagnostics

import org.slatelauncher.client.api.ModuleId

private const val DEFAULT_CAPACITY: Int = 200
private const val DEFAULT_MESSAGE_LIMIT: Int = 512
private const val MINIMUM_CAPACITY: Int = 1
private const val MAXIMUM_CAPACITY: Int = 2_000
private const val MINIMUM_MESSAGE_LIMIT: Int = 64
private const val MAXIMUM_MESSAGE_LIMIT: Int = 4_096

public class BoundedDiagnosticBuffer(
    private val capacity: Int = DEFAULT_CAPACITY,
    private val messageLimit: Int = DEFAULT_MESSAGE_LIMIT,
    private val privateValues: Set<String> = emptySet(),
) {
    private val records: ArrayDeque<DiagnosticRecord> = ArrayDeque(capacity)

    init {
        require(capacity in MINIMUM_CAPACITY..MAXIMUM_CAPACITY) {
            "diagnostic capacity must be between $MINIMUM_CAPACITY and $MAXIMUM_CAPACITY"
        }
        require(messageLimit in MINIMUM_MESSAGE_LIMIT..MAXIMUM_MESSAGE_LIMIT) {
            "diagnostic message limit must be between $MINIMUM_MESSAGE_LIMIT and $MAXIMUM_MESSAGE_LIMIT"
        }
    }

    @Synchronized
    public fun append(record: DiagnosticRecord) {
        if (records.size == capacity) {
            records.removeFirst()
        }
        records.addLast(record.copy(message = sanitize(record.message).take(messageLimit)))
    }

    @Synchronized
    public fun snapshot(): List<DiagnosticRecord> = records.toList()

    @Synchronized
    public fun health(moduleId: ModuleId): ModuleHealthSnapshot {
        val moduleRecords = records.filter { it.moduleId == moduleId }
        return ModuleHealthSnapshot(
            moduleId = moduleId,
            warningCount = moduleRecords.count { it.severity == DiagnosticSeverity.WARNING },
            errorCount = moduleRecords.count { it.severity == DiagnosticSeverity.ERROR },
            latestTimestampEpochMillis = moduleRecords.maxOfOrNull { it.timestampEpochMillis },
        )
    }

    private fun sanitize(message: String): String {
        var sanitized = message
        for (privateValue in privateValues.filter(
            String::isNotBlank,
        ).sortedByDescending(String::length)) {
            sanitized = sanitized.replace(privateValue, "<private>", ignoreCase = true)
        }
        for (marker in SECRET_MARKERS) {
            sanitized = redactMarker(sanitized, marker)
        }
        return sanitized.replace(CONTROL_CHARACTERS, " ")
    }

    private fun redactMarker(message: String, marker: String): String {
        val result = StringBuilder(message.length)
        var remainder = message
        while (true) {
            val index = remainder.indexOf(marker, ignoreCase = true)
            if (index < 0) {
                result.append(remainder)
                return result.toString()
            }
            val valueStart = index + marker.length
            result.append(remainder.substring(0, valueStart)).append("[redacted]")
            val tail = remainder.substring(valueStart)
            val valueEnd = tail.indexOfFirst { it.isWhitespace() || it == '&' || it == '"' }
            remainder = if (valueEnd < 0) "" else tail.substring(valueEnd)
        }
    }

    private companion object {
        val SECRET_MARKERS: List<String> =
            listOf("access_token=", "refresh_token=", "client_secret=", "authorization=", "Bearer ")
        val CONTROL_CHARACTERS: Regex = Regex("[\\u0000-\\u0008\\u000B\\u000C\\u000E-\\u001F]")
    }
}
