package org.slatelauncher.client.config

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertThrows
import org.junit.jupiter.api.Test
import org.slatelauncher.client.api.ModuleId

internal class ConfigReconcilerTest {
    private val moduleId = ModuleId.parse("slate.hud")
    private val schema =
        ConfigSchema(
            moduleId = moduleId,
            version = 2,
            fields =
                listOf(
                    ConfigField(
                        "showFps",
                        ConfigValueKind.BOOLEAN,
                        ConfigValue.BooleanValue(true),
                    ),
                    ConfigField(
                        "scale",
                        ConfigValueKind.NUMBER,
                        ConfigValue.NumberValue(1.0),
                    ),
                ),
        )

    @Test
    fun `patches known fields and preserves future fields`() {
        val current =
            ConfigDocument(
                moduleId,
                schemaVersion = 1,
                revision = 4,
                values = mapOf("futureField" to ConfigValue.TextValue("kept")),
            )

        val result =
            ConfigReconciler.applyPatch(
                current,
                schema,
                expectedRevision = 4,
                patch = mapOf("scale" to ConfigValue.NumberValue(1.25)),
            )

        assertEquals(5, result.document.revision)
        assertEquals(ConfigValue.NumberValue(1.25), result.document.values["scale"])
        assertEquals(ConfigValue.TextValue("kept"), result.document.values["futureField"])
        assertEquals(setOf("showFps"), result.defaultedFields)
        assertEquals(setOf("futureField"), result.preservedUnknownFields)
    }

    @Test
    fun `stale revisions cannot overwrite newer settings`() {
        val current = ConfigDocument(moduleId, 2, 8, emptyMap())
        assertThrows(RevisionConflictException::class.java) {
            ConfigReconciler.applyPatch(current, schema, 7, emptyMap())
        }
    }
}
