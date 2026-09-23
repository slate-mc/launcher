package org.slatelauncher.client.modules.pvp

private const val MILLIS_PER_SECOND: Long = 1_000
private const val MINIMUM_WINDOW_MILLIS: Long = 100
private const val MAXIMUM_WINDOW_MILLIS: Long = 10_000

public class ClickRateTracker(private val windowMillis: Long = MILLIS_PER_SECOND) {
    private val clicks: ArrayDeque<Long> = ArrayDeque()
    private var latestTimestamp: Long = -1

    init {
        require(windowMillis in MINIMUM_WINDOW_MILLIS..MAXIMUM_WINDOW_MILLIS) {
            "click-rate window must be between $MINIMUM_WINDOW_MILLIS and $MAXIMUM_WINDOW_MILLIS ms"
        }
    }

    @Synchronized
    public fun record(timestampMillis: Long): Int {
        require(timestampMillis >= latestTimestamp) { "click timestamps must be monotonic" }
        latestTimestamp = timestampMillis
        clicks.addLast(timestampMillis)
        evict(timestampMillis)
        return clicks.size
    }

    @Synchronized
    public fun clicksPerSecond(timestampMillis: Long): Double {
        require(timestampMillis >= latestTimestamp) { "click timestamps must be monotonic" }
        latestTimestamp = timestampMillis
        evict(timestampMillis)
        return clicks.size * (MILLIS_PER_SECOND.toDouble() / windowMillis)
    }

    private fun evict(nowMillis: Long) {
        val oldestAllowed = nowMillis - windowMillis
        while (clicks.firstOrNull()?.let { it <= oldestAllowed } == true) {
            clicks.removeFirst()
        }
    }
}
