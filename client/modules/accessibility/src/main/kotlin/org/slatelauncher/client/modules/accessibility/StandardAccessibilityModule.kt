package org.slatelauncher.client.modules.accessibility

import org.slatelauncher.client.api.ClientModule
import org.slatelauncher.client.api.ModuleCapability
import org.slatelauncher.client.api.ModuleContext
import org.slatelauncher.client.api.ModuleDescriptor
import org.slatelauncher.client.api.ModuleId
import org.slatelauncher.client.api.ModuleTarget
import org.slatelauncher.client.api.SemanticVersion

public class StandardAccessibilityModule(target: ModuleTarget) : ClientModule {
    override val descriptor: ModuleDescriptor =
        ModuleDescriptor(
            id = ModuleId.parse("slate.accessibility"),
            version = SemanticVersion(0, 1, 0),
            capabilities = setOf(ModuleCapability.ACCESSIBILITY, ModuleCapability.HUD),
            supportedTargets = setOf(target),
        )

    override fun start(context: ModuleContext) {
        context.events.emit("module.accessibility.started", "accessibility preferences applied")
    }

    override fun stop(context: ModuleContext) {
        context.events.emit("module.accessibility.stopped", "accessibility module stopped")
    }
}
