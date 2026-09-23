import java.security.MessageDigest
import org.apache.tools.ant.filters.ReplaceTokens
import org.gradle.api.DefaultTask
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.file.RegularFileProperty
import org.gradle.api.tasks.InputFile
import org.gradle.api.tasks.OutputDirectory
import org.gradle.api.tasks.PathSensitive
import org.gradle.api.tasks.PathSensitivity
import org.gradle.api.tasks.TaskAction

abstract class StageClientArtifactTask : DefaultTask() {
    @get:InputFile
    @get:PathSensitive(PathSensitivity.NONE)
    abstract val artifactFile: RegularFileProperty

    @get:InputFile
    @get:PathSensitive(PathSensitivity.NONE)
    abstract val fabricApiFile: RegularFileProperty

    @get:OutputDirectory
    abstract val outputDirectory: DirectoryProperty

    @TaskAction
    fun stage() {
        val remappedJar = artifactFile.get().asFile
        val destination = outputDirectory.get().asFile
        destination.deleteRecursively()
        destination.mkdirs()
        destination.resolve(".gitkeep").writeText("")
        val stagedName = "slate-client-fabric-1.21.1.jar"
        val stagedJar = destination.resolve(stagedName)
        remappedJar.copyTo(stagedJar, overwrite = true)
        val fabricApi = fabricApiFile.get().asFile
        val fabricApiName = "fabric-api-0.116.17+1.21.1.jar"
        val stagedFabricApi = destination.resolve(fabricApiName)
        fabricApi.copyTo(stagedFabricApi, overwrite = true)
        val clientSha256 =
            MessageDigest
                .getInstance("SHA-256")
                .digest(stagedJar.readBytes())
                .joinToString("") { byte -> "%02x".format(byte) }
        val fabricApiSha256 =
            MessageDigest
                .getInstance("SHA-256")
                .digest(stagedFabricApi.readBytes())
                .joinToString("") { byte -> "%02x".format(byte) }
        destination.resolve("manifest.json").writeText(
            """
            {
              "schema": 1,
              "artifacts": [
                {
                  "minecraftVersion": "1.21.1",
                  "loader": "fabric",
                  "loaderVersion": "0.19.5",
                  "file": "$stagedName",
                  "destination": "mods/slate-client.jar",
                  "sha256": "$clientSha256",
                  "displayName": "Slate Client"
                },
                {
                  "minecraftVersion": "1.21.1",
                  "loader": "fabric",
                  "loaderVersion": "0.19.5",
                  "file": "$fabricApiName",
                  "destination": "mods/$fabricApiName",
                  "sha256": "$fabricApiSha256",
                  "displayName": "Fabric API"
                }
              ]
            }
            """.trimIndent() + System.lineSeparator(),
        )
    }
}

plugins {
    `java-library`
    id("net.fabricmc.fabric-loom-remap")
}

val fabricApiCoordinate = "net.fabricmc.fabric-api:fabric-api:0.116.17+1.21.1"
val desktopFabricApi by configurations.creating {
    isCanBeConsumed = false
    isCanBeResolved = true
    isTransitive = false
}

base {
    archivesName.set("slate-client-fabric-1.21.1")
}

dependencies {
    minecraft("com.mojang:minecraft:1.21.1")
    mappings(loom.officialMojangMappings())

    modImplementation("net.fabricmc:fabric-loader:0.19.5")
    modImplementation(fabricApiCoordinate)
    desktopFabricApi(fabricApiCoordinate)
    val fabricLanguageKotlin = "net.fabricmc:fabric-language-kotlin:1.13.11+kotlin.2.3.21"
    modImplementation(fabricLanguageKotlin)
    include(fabricLanguageKotlin)
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.11.0")

    api(project(":client-api"))
    implementation(project(":client-runtime"))
    implementation(project(":client-diagnostics"))
    implementation(project(":client-hud"))
    implementation(project(":client-modules-performance"))
    implementation(project(":client-modules-accessibility"))
    implementation(project(":client-modules-qol"))
    implementation(project(":client-modules-visual"))
    implementation(project(":client-modules-pvp"))

    include(project(":client-api"))
    include(project(":client-runtime"))
    include(project(":client-diagnostics"))
    include(project(":client-hud"))
    include(project(":client-modules-performance"))
    include(project(":client-modules-accessibility"))
    include(project(":client-modules-qol"))
    include(project(":client-modules-visual"))
    include(project(":client-modules-pvp"))
}

val modVersion = project.version.toString()

tasks.processResources {
    inputs.property("version", modVersion)
    filter<ReplaceTokens>("tokens" to mapOf("version" to modVersion))
}

val stagedDesktopArtifacts =
    rootProject.layout.projectDirectory.dir("desktop-resources/client")

tasks.register<StageClientArtifactTask>("stageDesktopClientArtifacts") {
    group = "distribution"
    description = "Builds and stages the verified Fabric Slate Client artifact for Tauri."
    val remapJar = tasks.named("remapJar")
    dependsOn(remapJar)
    artifactFile.set(layout.file(remapJar.map { task -> task.outputs.files.singleFile }))
    fabricApiFile.set(layout.file(providers.provider { desktopFabricApi.singleFile }))
    outputDirectory.set(stagedDesktopArtifacts)
}
