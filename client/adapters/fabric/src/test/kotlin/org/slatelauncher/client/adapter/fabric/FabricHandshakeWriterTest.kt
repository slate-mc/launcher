package org.slatelauncher.client.adapter.fabric

import java.nio.file.Files
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.io.TempDir
import org.slatelauncher.client.api.ActiveModule
import org.slatelauncher.client.api.AdapterStatus
import org.slatelauncher.client.api.ClientHandshake
import org.slatelauncher.client.api.GameTarget
import org.slatelauncher.client.api.LoaderKind
import org.slatelauncher.client.api.ModuleId
import org.slatelauncher.client.api.SemanticVersion

class FabricHandshakeWriterTest {
    @TempDir lateinit var gameDirectory: java.nio.file.Path

    @Test
    fun `writes bounded launcher-readable handshake then deletes it`() {
        val writer = FabricHandshakeWriter(gameDirectory)
        val handshake =
            ClientHandshake(
                protocolVersion = 1,
                target = GameTarget("1.21.1", LoaderKind.FABRIC, "0.19.5", 21),
                adapterStatus = AdapterStatus.CONTRACT_ONLY,
                modules =
                    listOf(
                        ActiveModule(ModuleId.parse("slate.qol"), SemanticVersion(0, 1, 0)),
                    ),
            )

        writer.write(handshake, processId = 42, startedAtEpochMillis = 1_000)

        val path = gameDirectory.resolve("slate/client-handshake.json")
        val contents = Files.readString(path)
        val document = Json.parseToJsonElement(contents).jsonObject
        assertTrue(document.getValue("schema").jsonPrimitive.content == "1")
        assertTrue(document.getValue("adapterStatus").jsonPrimitive.content == "contract_only")
        val target = document.getValue("target").jsonObject
        assertTrue(target.getValue("loaderVersion").jsonPrimitive.content == "0.19.5")
        assertTrue(contents.contains("slate.qol"))

        writer.delete()
        assertFalse(Files.exists(path))
    }
}
