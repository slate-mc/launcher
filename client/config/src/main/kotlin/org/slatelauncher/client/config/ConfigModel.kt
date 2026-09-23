package org.slatelauncher.client.config

import org.slatelauncher.client.api.ModuleId

private const val MAXIMUM_CONFIG_TEXT_LENGTH: Int = 4_096

public enum class ConfigValueKind {
    BOOLEAN,
    NUMBER,
    TEXT,
}

public sealed interface ConfigValue {
    public val kind: ConfigValueKind

    public data class BooleanValue(public val value: Boolean) : ConfigValue {
        override val kind: ConfigValueKind = ConfigValueKind.BOOLEAN
    }

    public data class NumberValue(public val value: Double) : ConfigValue {
        init {
            require(value.isFinite()) { "configuration numbers must be finite" }
        }

        override val kind: ConfigValueKind = ConfigValueKind.NUMBER
    }

    public data class TextValue(public val value: String) : ConfigValue {
        init {
            require(value.length <= MAXIMUM_CONFIG_TEXT_LENGTH) { "configuration text is too long" }
        }

        override val kind: ConfigValueKind = ConfigValueKind.TEXT
    }
}

public data class ConfigField(
    public val key: String,
    public val kind: ConfigValueKind,
    public val defaultValue: ConfigValue,
) {
    init {
        require(KEY_PATTERN.matches(key)) { "configuration field key is invalid" }
        require(defaultValue.kind == kind) { "configuration default has the wrong type" }
    }

    private companion object {
        val KEY_PATTERN: Regex = Regex("[a-z][a-zA-Z0-9.]{1,63}")
    }
}

public data class ConfigSchema(
    public val moduleId: ModuleId,
    public val version: Int,
    public val fields: List<ConfigField>,
    public val preserveUnknownFields: Boolean = true,
) {
    init {
        require(version > 0) { "schema version must be positive" }
        require(fields.map { it.key }.toSet().size == fields.size) { "field keys must be unique" }
    }
}

public data class ConfigDocument(
    public val moduleId: ModuleId,
    public val schemaVersion: Int,
    public val revision: Long,
    public val values: Map<String, ConfigValue>,
) {
    init {
        require(schemaVersion > 0) { "schema version must be positive" }
        require(revision >= 0) { "revision cannot be negative" }
    }
}

public data class ReconciliationResult(
    public val document: ConfigDocument,
    public val defaultedFields: Set<String>,
    public val preservedUnknownFields: Set<String>,
)

public class RevisionConflictException(expected: Long, actual: Long) :
    IllegalStateException("configuration revision conflict: expected $expected, found $actual")
