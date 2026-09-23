package org.slatelauncher.client.adapter.fabric

import java.time.Clock
import java.util.concurrent.atomic.AtomicBoolean
import net.fabricmc.api.ClientModInitializer
import net.fabricmc.loader.api.FabricLoader
import org.slatelauncher.client.adapter.fabric.ui.SlateScreenController
import org.slatelauncher.client.api.GameTarget
import org.slatelauncher.client.api.LoaderKind
import org.slatelauncher.client.api.ModuleContext
import org.slatelauncher.client.api.ModuleEventSink
import org.slatelauncher.client.api.ModuleTarget
import org.slatelauncher.client.modules.accessibility.StandardAccessibilityModule
import org.slatelauncher.client.modules.performance.StandardPerformanceModule
import org.slatelauncher.client.modules.pvp.StandardPvpModule
import org.slatelauncher.client.modules.qol.StandardQolModule
import org.slatelauncher.client.modules.visual.StandardVisualModule
import org.slatelauncher.client.runtime.ModuleSupervisor
import org.slf4j.LoggerFactory

public class SlateFabricClient : ClientModInitializer {
    override fun onInitializeClient() {
        check(STARTED.compareAndSet(false, true)) { "Slate Client initialized more than once" }

        val loader = FabricLoader.getInstance()
        val minecraftVersion = loader.requiredVersionOf("minecraft")
        val loaderVersion = loader.requiredVersionOf("fabricloader")
        require(minecraftVersion == MINECRAFT_VERSION) {
            "Slate Client requires Minecraft $MINECRAFT_VERSION"
        }
        require(loaderVersion == FABRIC_LOADER_VERSION) {
            "Slate Client requires Fabric Loader $FABRIC_LOADER_VERSION"
        }

        val target =
            GameTarget(
                minecraftVersion = minecraftVersion,
                loader = LoaderKind.FABRIC,
                loaderVersion = loaderVersion,
                javaMajor = Runtime.version().feature(),
            )
        val moduleTarget =
            ModuleTarget(
                minecraftVersion = target.minecraftVersion,
                loader = target.loader,
                loaderVersion = target.loaderVersion,
            )
        val modules =
            listOf(
                StandardPerformanceModule(moduleTarget),
                StandardAccessibilityModule(moduleTarget),
                StandardQolModule(moduleTarget),
                StandardVisualModule(moduleTarget),
                StandardPvpModule(moduleTarget),
            )
        val context = ModuleContext(target, loggingEventSink())
        val supervisor = ModuleSupervisor(modules, context)
        val writer = FabricHandshakeWriter(loader.gameDir)

        writer.delete()
        supervisor.startAll()
        val handshake =
            FabricContractAdapter(MINECRAFT_VERSION, FABRIC_LOADER_VERSION)
                .handshake(target, modules.map { it.descriptor })
        writer.write(
            handshake = handshake,
            processId = ProcessHandle.current().pid(),
            startedAtEpochMillis = Clock.systemUTC().millis(),
        )
        SlateScreenController(
            supervisor = supervisor,
            clientVersion = loader.requiredVersionOf("slate-client"),
        ).register()
        registerShutdown(supervisor, writer)
        LOGGER.info(
            "Slate Client Fabric bootstrap completed with {} contract modules",
            handshake.modules.size,
        )
    }

    private fun loggingEventSink(): ModuleEventSink = ModuleEventSink { code, detail ->
        LOGGER.info("{}: {}", code, detail)
    }

    private fun registerShutdown(supervisor: ModuleSupervisor, writer: FabricHandshakeWriter) {
        Runtime.getRuntime().addShutdownHook(
            Thread(
                {
                    runCatching(supervisor::stopAll)
                        .onFailure { error -> LOGGER.warn("Slate Client shutdown failed", error) }
                    runCatching(writer::delete)
                        .onFailure { error ->
                            LOGGER.warn("Could not remove Slate Client handshake", error)
                        }
                },
                "slate-client-shutdown",
            ),
        )
    }

    private companion object {
        const val MINECRAFT_VERSION: String = "1.21.1"
        const val FABRIC_LOADER_VERSION: String = "0.19.5"
        val STARTED: AtomicBoolean = AtomicBoolean()
        val LOGGER = LoggerFactory.getLogger("SlateClient")
    }
}

private fun FabricLoader.requiredVersionOf(modId: String): String = getModContainer(modId)
    .orElseThrow { IllegalStateException("required mod $modId is unavailable") }
    .metadata
    .version
    .friendlyString
