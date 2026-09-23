package org.slatelauncher.client.api

public enum class AdapterStatus {
    CONTRACT_ONLY,
    ACTIVE,
}

public data class ActiveModule(public val id: ModuleId, public val version: SemanticVersion)

public data class ClientHandshake(
    public val protocolVersion: Int,
    public val target: GameTarget,
    public val adapterStatus: AdapterStatus,
    public val modules: List<ActiveModule>,
)

public interface LoaderAdapter {
    public val loader: LoaderKind

    public fun handshake(target: GameTarget, modules: Collection<ModuleDescriptor>): ClientHandshake
}
