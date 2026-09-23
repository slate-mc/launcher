package org.slatelauncher.client.adapter.fabric

import java.nio.charset.StandardCharsets
import java.nio.file.AtomicMoveNotSupportedException
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.StandardCopyOption
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.buildJsonArray
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import kotlinx.serialization.json.putJsonObject
import org.slatelauncher.client.api.ClientHandshake

internal class FabricHandshakeWriter(private val gameDirectory: Path) {
    private val handshakeDirectory: Path = gameDirectory.resolve("slate")
    private val handshakePath: Path = handshakeDirectory.resolve("client-handshake.json")

    fun write(handshake: ClientHandshake, processId: Long, startedAtEpochMillis: Long) {
        require(processId > 0) { "process ID must be positive" }
        require(startedAtEpochMillis >= 0) { "startup timestamp cannot be negative" }
        Files.createDirectories(handshakeDirectory)
        val temporaryPath = Files.createTempFile(handshakeDirectory, "client-handshake-", ".tmp")
        try {
            Files.writeString(
                temporaryPath,
                handshake.toJson(processId, startedAtEpochMillis),
                StandardCharsets.UTF_8,
            )
            moveAtomically(temporaryPath, handshakePath)
        } finally {
            Files.deleteIfExists(temporaryPath)
        }
    }

    fun delete() {
        Files.deleteIfExists(handshakePath)
    }

    private fun moveAtomically(source: Path, destination: Path) {
        try {
            Files.move(
                source,
                destination,
                StandardCopyOption.ATOMIC_MOVE,
                StandardCopyOption.REPLACE_EXISTING,
            )
        } catch (_: AtomicMoveNotSupportedException) {
            Files.move(source, destination, StandardCopyOption.REPLACE_EXISTING)
        }
    }
}

private fun ClientHandshake.toJson(processId: Long, startedAtEpochMillis: Long): String {
    val document = buildJsonObject {
        put("schema", HANDSHAKE_SCHEMA)
        put("protocolVersion", protocolVersion)
        put("processId", processId)
        put("startedAtEpochMillis", startedAtEpochMillis)
        put("adapterStatus", adapterStatus.name.lowercase())
        putJsonObject("target") {
            put("minecraftVersion", target.minecraftVersion)
            put("loader", target.loader.name.lowercase())
            put("loaderVersion", target.loaderVersion)
            put("javaMajor", target.javaMajor)
        }
        put(
            "modules",
            buildJsonArray {
                for (module in modules) {
                    add(
                        buildJsonObject {
                            put("id", module.id.toString())
                            put("version", module.version.toString())
                        },
                    )
                }
            },
        )
    }
    return HANDSHAKE_JSON.encodeToString(document) + "\n"
}

private const val HANDSHAKE_SCHEMA: Int = 1
private val HANDSHAKE_JSON: Json = Json { prettyPrint = true }
