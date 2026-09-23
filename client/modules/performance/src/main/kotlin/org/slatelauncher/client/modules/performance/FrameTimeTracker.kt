package org.slatelauncher.client.modules.performance

import kotlin.math.ceil

private const val DEFAULT_SAMPLE_CAPACITY: Int = 240
private const val MINIMUM_SAMPLE_CAPACITY: Int = 30
private const val MAXIMUM_SAMPLE_CAPACITY: Int = 10_000
private const val PERCENTILE_95: Double = 0.95

public data class FrameTimeSummary(
    public val sampleCount: Int,
    public val averageMillis: Double,
    public val percentile95Millis: Double,
    public val worstMillis: Double,
)

public class FrameTimeTracker(private val capacity: Int = DEFAULT_SAMPLE_CAPACITY) {
    private val samples: ArrayDeque<Double> = ArrayDeque(capacity)

    init {
        require(capacity in MINIMUM_SAMPLE_CAPACITY..MAXIMUM_SAMPLE_CAPACITY) {
            "frame-time capacity must be between $MINIMUM_SAMPLE_CAPACITY and $MAXIMUM_SAMPLE_CAPACITY"
        }
    }

    @Synchronized
    public fun record(frameTimeMillis: Double) {
        require(frameTimeMillis >= 0.0 && frameTimeMillis.isFinite()) {
            "frame time must be finite and non-negative"
        }
        if (samples.size == capacity) {
            samples.removeFirst()
        }
        samples.addLast(frameTimeMillis)
    }

    @Synchronized
    public fun summary(): FrameTimeSummary? {
        if (samples.isEmpty()) {
            return null
        }
        val sorted = samples.sorted()
        val percentileIndex = (ceil(sorted.size * PERCENTILE_95).toInt() - 1).coerceAtLeast(0)
        return FrameTimeSummary(
            sampleCount = sorted.size,
            averageMillis = sorted.average(),
            percentile95Millis = sorted[percentileIndex],
            worstMillis = sorted.last(),
        )
    }
}
