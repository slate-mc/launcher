package org.slatelauncher.client.modules.qol

import org.slatelauncher.client.api.ClientModule
import org.slatelauncher.client.api.ModuleCapability
import org.slatelauncher.client.api.ModuleContext
import org.slatelauncher.client.api.ModuleDescriptor
import org.slatelauncher.client.api.ModuleId
import org.slatelauncher.client.api.ModuleTarget
import org.slatelauncher.client.api.SemanticVersion

public class StandardQolModule(target: ModuleTarget) : ClientModule {
    override val descriptor: ModuleDescriptor =
        ModuleDescriptor(
            id = ModuleId.parse("slate.qol"),
            version = SemanticVersion(0, 1, 0),
            capabilities = setOf(ModuleCapability.QOL, ModuleCapability.HUD),
            supportedTargets = setOf(target),
        )

    override fun start(context: ModuleContext) {
        context.events.emit("module.qol.started", "quality-of-life tools started")
    }

    override fun stop(context: ModuleContext) {
        context.events.emit("module.qol.stopped", "quality-of-life tools stopped")
    }
}
