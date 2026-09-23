package org.slatelauncher.client.api

public const val SLATE_CLIENT_PROTOCOL_VERSION: Int = 1
private const val MINIMUM_SUPPORTED_JAVA_MAJOR: Int = 8

public enum class LoaderKind {
    FABRIC,
    NEOFORGE,
}

public enum class ModuleCapability {
    HUD,
    PERFORMANCE,
    ACCESSIBILITY,
    VISUAL,
    QOL,
    PVP,
    PROFILE,
    DIAGNOSTICS,
    CREATOR,
}

public data class SemanticVersion(
    public val major: Int,
    public val minor: Int,
    public val patch: Int,
) : Comparable<SemanticVersion> {
    init {
        require(major >= 0 && minor >= 0 && patch >= 0) { "version components must be positive" }
    }

    override fun compareTo(other: SemanticVersion): Int = compareValuesBy(
        this,
        other,
        SemanticVersion::major,
        SemanticVersion::minor,
        SemanticVersion::patch,
    )

    override fun toString(): String = "$major.$minor.$patch"
}

public data class GameTarget(
    public val minecraftVersion: String,
    public val loader: LoaderKind,
    public val loaderVersion: String,
    public val javaMajor: Int,
) {
    init {
        require(minecraftVersion.isNotBlank()) { "Minecraft version is required" }
        require(loaderVersion.isNotBlank()) { "loader version is required" }
        require(javaMajor >= MINIMUM_SUPPORTED_JAVA_MAJOR) {
            "Java major must be at least $MINIMUM_SUPPORTED_JAVA_MAJOR"
        }
    }
}

public data class ModuleTarget(
    public val minecraftVersion: String,
    public val loader: LoaderKind,
    public val loaderVersion: String,
) {
    public fun matches(target: GameTarget): Boolean = minecraftVersion == target.minecraftVersion &&
        loader == target.loader &&
        loaderVersion == target.loaderVersion
}

public data class ModuleDescriptor(
    public val id: ModuleId,
    public val version: SemanticVersion,
    public val protocolVersion: Int = SLATE_CLIENT_PROTOCOL_VERSION,
    public val capabilities: Set<ModuleCapability>,
    public val dependencies: Set<ModuleId> = emptySet(),
    public val supportedTargets: Set<ModuleTarget>,
) {
    init {
        require(protocolVersion > 0) { "protocol version must be positive" }
        require(supportedTargets.isNotEmpty()) { "at least one exact game target is required" }
        require(id !in dependencies) { "a module cannot depend on itself" }
    }

    public fun supports(target: GameTarget): Boolean = supportedTargets.any { it.matches(target) }
}

public fun interface ModuleEventSink {
    public fun emit(code: String, detail: String)
}

public data class ModuleContext(public val target: GameTarget, public val events: ModuleEventSink)

public interface ClientModule {
    public val descriptor: ModuleDescriptor

    public fun start(context: ModuleContext)

    public fun stop(context: ModuleContext)
}

public enum class ModuleState {
    REGISTERED,
    STARTING,
    ACTIVE,
    STOPPING,
    STOPPED,
    FAILED,
}
