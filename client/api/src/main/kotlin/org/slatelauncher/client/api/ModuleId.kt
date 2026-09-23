package org.slatelauncher.client.api

@JvmInline
public value class ModuleId private constructor(public val value: String) : Comparable<ModuleId> {
    override fun compareTo(other: ModuleId): Int = value.compareTo(other.value)

    override fun toString(): String = value

    public companion object {
        private val VALID_ID = Regex("[a-z][a-z0-9._-]{2,63}")

        public fun parse(value: String): ModuleId {
            require(VALID_ID.matches(value)) {
                "module IDs must contain 3-64 lowercase letters, numbers, dots, dashes, or underscores"
            }
            return ModuleId(value)
        }
    }
}
