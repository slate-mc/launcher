package org.slatelauncher.client.runtime

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertThrows
import org.junit.jupiter.api.Test
import org.slatelauncher.client.api.ClientModule
import org.slatelauncher.client.api.GameTarget
import org.slatelauncher.client.api.LoaderKind
import org.slatelauncher.client.api.ModuleCapability
import org.slatelauncher.client.api.ModuleContext
import org.slatelauncher.client.api.ModuleDescriptor
import org.slatelauncher.client.api.ModuleEventSink
import org.slatelauncher.client.api.ModuleId
import org.slatelauncher.client.api.ModuleState
import org.slatelauncher.client.api.ModuleTarget
import org.slatelauncher.client.api.SemanticVersion

internal class ModuleSupervisorTest {
    private val target = GameTarget("1.21.1", LoaderKind.FABRIC, "0.16.14", 21)
    private val events = mutableListOf<String>()
    private val context = ModuleContext(target, ModuleEventSink { code, _ -> events += code })

    @Test
    fun `dependencies start first and stop last`() {
        val base = FakeModule("slate.base")
        val hud = FakeModule("slate.hud", setOf(base.descriptor.id))
        val supervisor = ModuleSupervisor(listOf(hud, base), context)

        assertEquals(listOf(base.descriptor.id, hud.descriptor.id), supervisor.startAll())
        assertEquals(listOf(hud.descriptor.id, base.descriptor.id), supervisor.stopAll())
        assertEquals(
            listOf("start:slate.base", "start:slate.hud", "stop:slate.hud", "stop:slate.base"),
            events,
        )
    }

    @Test
    fun `missing dependencies fail before modules start`() {
        val module = FakeModule("slate.hud", setOf(ModuleId.parse("slate.missing")))
        val supervisor = ModuleSupervisor(listOf(module), context)

        assertThrows(ModuleLifecycleException::class.java) { supervisor.startAll() }
        assertEquals(ModuleState.REGISTERED, supervisor.snapshot()[module.descriptor.id])
    }

    @Test
    fun `module controls start dependencies and stop dependents`() {
        val base = FakeModule("slate.base")
        val hud = FakeModule("slate.hud", setOf(base.descriptor.id))
        val supervisor = ModuleSupervisor(listOf(hud, base), context)

        supervisor.setEnabled(hud.descriptor.id, true)
        assertEquals(ModuleState.ACTIVE, supervisor.snapshot()[base.descriptor.id])
        assertEquals(ModuleState.ACTIVE, supervisor.snapshot()[hud.descriptor.id])

        supervisor.setEnabled(base.descriptor.id, false)
        assertEquals(ModuleState.STOPPED, supervisor.snapshot()[base.descriptor.id])
        assertEquals(ModuleState.STOPPED, supervisor.snapshot()[hud.descriptor.id])
        assertEquals(
            listOf("start:slate.base", "start:slate.hud", "stop:slate.hud", "stop:slate.base"),
            events,
        )
    }

    private inner class FakeModule(id: String, dependencies: Set<ModuleId> = emptySet()) :
        ClientModule {
        override val descriptor: ModuleDescriptor =
            ModuleDescriptor(
                id = ModuleId.parse(id),
                version = SemanticVersion(1, 0, 0),
                capabilities = setOf(ModuleCapability.DIAGNOSTICS),
                dependencies = dependencies,
                supportedTargets = setOf(ModuleTarget("1.21.1", LoaderKind.FABRIC, "0.16.14")),
            )

        override fun start(context: ModuleContext) {
            context.events.emit("start:${descriptor.id}", "started")
        }

        override fun stop(context: ModuleContext) {
            context.events.emit("stop:${descriptor.id}", "stopped")
        }
    }
}
