package org.slatelauncher.client.adapter.fabric

import org.slatelauncher.client.api.ActiveModule
import org.slatelauncher.client.api.AdapterStatus
import org.slatelauncher.client.api.ClientHandshake
import org.slatelauncher.client.api.GameTarget
import org.slatelauncher.client.api.LoaderAdapter
import org.slatelauncher.client.api.LoaderKind
import org.slatelauncher.client.api.ModuleDescriptor
import org.slatelauncher.client.api.SLATE_CLIENT_PROTOCOL_VERSION

public class FabricContractAdapter(
    private val minecraftVersion: String,
    private val loaderVersion: String,
) : LoaderAdapter {
    override val loader: LoaderKind = LoaderKind.FABRIC

    override fun handshake(
        target: GameTarget,
        modules: Collection<ModuleDescriptor>,
    ): ClientHandshake {
        require(target.loader == loader) { "Fabric adapter cannot serve ${target.loader}" }
        require(
            target.minecraftVersion == minecraftVersion && target.loaderVersion == loaderVersion,
        ) {
            "Fabric adapter target does not match its exact build target"
        }
        require(modules.all { it.supports(target) }) {
            "one or more Slate Client modules do not support this Fabric target"
        }
        return ClientHandshake(
            protocolVersion = SLATE_CLIENT_PROTOCOL_VERSION,
            target = target,
            adapterStatus = AdapterStatus.CONTRACT_ONLY,
            modules = modules.sortedBy { it.id }.map { ActiveModule(it.id, it.version) },
        )
    }
}
