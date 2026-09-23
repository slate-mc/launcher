package org.slatelauncher.client.modules.performance

import org.slatelauncher.client.api.ClientModule
import org.slatelauncher.client.api.ModuleCapability
import org.slatelauncher.client.api.ModuleContext
import org.slatelauncher.client.api.ModuleDescriptor
import org.slatelauncher.client.api.ModuleId
import org.slatelauncher.client.api.ModuleTarget
import org.slatelauncher.client.api.SemanticVersion

public class StandardPerformanceModule(target: ModuleTarget) : ClientModule {
    override val descriptor: ModuleDescriptor =
        ModuleDescriptor(
            id = ModuleId.parse("slate.performance"),
            version = SemanticVersion(0, 1, 0),
            capabilities = setOf(ModuleCapability.PERFORMANCE, ModuleCapability.HUD),
            supportedTargets = setOf(target),
        )

    override fun start(context: ModuleContext) {
        context.events.emit("module.performance.started", "frame-time sampling is active")
    }

    override fun stop(context: ModuleContext) {
        context.events.emit("module.performance.stopped", "frame-time sampling stopped")
    }
}
