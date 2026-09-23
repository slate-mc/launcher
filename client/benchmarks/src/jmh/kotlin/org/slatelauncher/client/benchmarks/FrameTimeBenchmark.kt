package org.slatelauncher.client.benchmarks

import org.openjdk.jmh.annotations.Benchmark
import org.openjdk.jmh.annotations.Scope
import org.openjdk.jmh.annotations.State
import org.slatelauncher.client.modules.performance.FrameTimeSummary
import org.slatelauncher.client.modules.performance.FrameTimeTracker

@State(Scope.Thread)
public open class FrameTimeBenchmark {
    private val tracker = FrameTimeTracker()

    init {
        repeat(240) { tracker.record(4.0 + (it % 10)) }
    }

    @Benchmark
    public fun summarize(): FrameTimeSummary = requireNotNull(tracker.summary())
}
