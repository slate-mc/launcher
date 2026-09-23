package org.slatelauncher.client.benchmarks

import org.openjdk.jmh.annotations.Benchmark
import org.openjdk.jmh.annotations.Scope
import org.openjdk.jmh.annotations.State
import org.slatelauncher.client.hud.HudAnchor
import org.slatelauncher.client.hud.HudElementId
import org.slatelauncher.client.hud.HudElementLayout
import org.slatelauncher.client.hud.HudLayoutEngine
import org.slatelauncher.client.hud.ResolvedHudElement
import org.slatelauncher.client.hud.Viewport

@State(Scope.Thread)
public open class HudLayoutBenchmark {
    private val viewport = Viewport(1_920.0, 1_080.0)
    private val layout =
        HudElementLayout(
            HudElementId.parse("slate.performance"),
            HudAnchor.TOP_RIGHT,
            -16.0,
            16.0,
            120.0,
            24.0,
        )

    @Benchmark
    public fun resolve(): ResolvedHudElement = HudLayoutEngine.resolve(layout, viewport)
}
