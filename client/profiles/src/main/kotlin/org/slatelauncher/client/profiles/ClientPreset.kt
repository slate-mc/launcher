package org.slatelauncher.client.profiles

import org.slatelauncher.client.api.ModuleId
import org.slatelauncher.client.config.ConfigValue

private const val MAXIMUM_PRESET_NAME_LENGTH: Int = 64

public enum class PresetKind {
    DEFAULT,
    PERFORMANCE,
    SURVIVAL,
    MODDED,
    CREATOR,
    PVP,
}

public data class ClientPreset(
    public val id: String,
    public val name: String,
    public val kind: PresetKind,
    public val enabledModules: Set<ModuleId>,
    public val moduleConfiguration: Map<ModuleId, Map<String, ConfigValue>>,
) {
    init {
        require(ID_PATTERN.matches(id)) { "preset ID is invalid" }
        require(name.isNotBlank() && name.length <= MAXIMUM_PRESET_NAME_LENGTH) {
            "preset name is invalid"
        }
        require(moduleConfiguration.keys.all { it in enabledModules }) {
            "preset configuration references a disabled module"
        }
    }

    private companion object {
        val ID_PATTERN: Regex = Regex("[a-z][a-z0-9._-]{2,63}")
    }
}

public object StandardClientPresets {
    private val PERFORMANCE = ModuleId.parse("slate.performance")
    private val ACCESSIBILITY = ModuleId.parse("slate.accessibility")
    private val VISUAL = ModuleId.parse("slate.visual")
    private val QOL = ModuleId.parse("slate.qol")
    private val PVP = ModuleId.parse("slate.pvp")

    public fun defaults(): List<ClientPreset> = listOf(
        preset(
            "slate.default",
            "Default",
            PresetKind.DEFAULT,
            listOf(PERFORMANCE, ACCESSIBILITY, QOL),
        ),
        preset(
            "slate.performance",
            "Performance",
            PresetKind.PERFORMANCE,
            listOf(PERFORMANCE),
        ),
        preset(
            "slate.survival",
            "Survival",
            PresetKind.SURVIVAL,
            listOf(PERFORMANCE, ACCESSIBILITY, QOL),
        ),
        preset(
            "slate.modded",
            "Modded",
            PresetKind.MODDED,
            listOf(PERFORMANCE, ACCESSIBILITY),
        ),
        preset(
            "slate.creator",
            "Creator",
            PresetKind.CREATOR,
            listOf(PERFORMANCE, VISUAL, QOL),
        ),
        preset(
            "slate.pvp",
            "PvP",
            PresetKind.PVP,
            listOf(PERFORMANCE, VISUAL, QOL, PVP),
        ),
    )

    private fun preset(
        id: String,
        name: String,
        kind: PresetKind,
        modules: List<ModuleId>,
    ): ClientPreset = ClientPreset(id, name, kind, modules.toSet(), emptyMap())
}
