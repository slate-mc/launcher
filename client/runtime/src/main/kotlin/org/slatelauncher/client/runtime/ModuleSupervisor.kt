package org.slatelauncher.client.runtime

import org.slatelauncher.client.api.ClientModule
import org.slatelauncher.client.api.ModuleContext
import org.slatelauncher.client.api.ModuleId
import org.slatelauncher.client.api.ModuleState

public class ModuleSupervisor(
    modules: Collection<ClientModule>,
    private val context: ModuleContext,
) {
    private val modulesById: Map<ModuleId, ClientModule> = modules.associateUniqueById()
    private val states: MutableMap<ModuleId, ModuleState> =
        modulesById.keys.associateWithTo(linkedMapOf()) { ModuleState.REGISTERED }

    @Synchronized
    @Suppress("TooGenericExceptionCaught")
    public fun startAll(): List<ModuleId> {
        val started = mutableListOf<ClientModule>()
        try {
            for (module in dependencyOrder()) {
                require(module.descriptor.supports(context.target)) {
                    "module ${module.descriptor.id} does not support the exact game target"
                }
                states[module.descriptor.id] = ModuleState.STARTING
                module.start(context)
                states[module.descriptor.id] = ModuleState.ACTIVE
                started += module
            }
        } catch (error: Exception) {
            started.asReversed().forEach { stopAfterFailure(it) }
            throw ModuleLifecycleException("Slate Client module startup failed", error)
        }
        return started.map { it.descriptor.id }
    }

    @Synchronized
    @Suppress("TooGenericExceptionCaught")
    public fun stopAll(): List<ModuleId> {
        val stopped = mutableListOf<ModuleId>()
        for (module in dependencyOrder().asReversed()) {
            if (states[module.descriptor.id] != ModuleState.ACTIVE) {
                continue
            }
            states[module.descriptor.id] = ModuleState.STOPPING
            try {
                module.stop(context)
                states[module.descriptor.id] = ModuleState.STOPPED
                stopped += module.descriptor.id
            } catch (error: Exception) {
                states[module.descriptor.id] = ModuleState.FAILED
                throw ModuleLifecycleException("Slate Client module shutdown failed", error)
            }
        }
        return stopped
    }

    @Synchronized
    public fun snapshot(): Map<ModuleId, ModuleState> = states.toMap()

    private fun dependencyOrder(): List<ClientModule> {
        val order = mutableListOf<ClientModule>()
        val visiting = mutableSetOf<ModuleId>()
        val visited = mutableSetOf<ModuleId>()

        fun visit(module: ClientModule) {
            val id = module.descriptor.id
            if (id in visited) {
                return
            }
            check(visiting.add(id)) { "module dependency cycle includes $id" }
            for (dependencyId in module.descriptor.dependencies.sorted()) {
                val dependency = modulesById[dependencyId]
                    ?: error("module $id requires missing dependency $dependencyId")
                visit(dependency)
            }
            visiting.remove(id)
            visited += id
            order += module
        }

        modulesById.values.sortedBy { it.descriptor.id }.forEach(::visit)
        return order
    }

    @Suppress("TooGenericExceptionCaught")
    private fun stopAfterFailure(module: ClientModule) {
        val id = module.descriptor.id
        states[id] = ModuleState.STOPPING
        try {
            module.stop(context)
            states[id] = ModuleState.STOPPED
        } catch (_: Exception) {
            states[id] = ModuleState.FAILED
        }
    }
}

public class ModuleLifecycleException(message: String, cause: Throwable) :
    IllegalStateException(message, cause)

private fun Collection<ClientModule>.associateUniqueById(): Map<ModuleId, ClientModule> {
    val modules = linkedMapOf<ModuleId, ClientModule>()
    for (module in this) {
        require(modules.put(module.descriptor.id, module) == null) {
            "duplicate module ID ${module.descriptor.id}"
        }
    }
    return modules
}
