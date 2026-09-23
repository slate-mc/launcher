package org.slatelauncher.client.modules.pvp

import org.slatelauncher.client.api.ClientModule
import org.slatelauncher.client.api.ModuleCapability
import org.slatelauncher.client.api.ModuleContext
import org.slatelauncher.client.api.ModuleDescriptor
import org.slatelauncher.client.api.ModuleId
import org.slatelauncher.client.api.ModuleTarget
import org.slatelauncher.client.api.SemanticVersion

public class StandardPvpModule(target: ModuleTarget) : ClientModule {
    override val descriptor: ModuleDescriptor =
        ModuleDescriptor(
            id = ModuleId.parse("slate.pvp"),
            version = SemanticVersion(0, 1, 0),
            capabilities = setOf(ModuleCapability.PVP, ModuleCapability.HUD),
            supportedTargets = setOf(target),
        )

    override fun start(context: ModuleContext) {
        context.events.emit("module.pvp.started", "PvP HUD tools started")
    }

    override fun stop(context: ModuleContext) {
        context.events.emit("module.pvp.stopped", "PvP HUD tools stopped")
    }
}
