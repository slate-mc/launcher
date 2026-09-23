import org.apache.tools.ant.filters.ReplaceTokens

plugins {
    `java-library`
    id("net.fabricmc.fabric-loom-remap")
}

base {
    archivesName.set("slate-client-fabric-1.21.1")
}

dependencies {
    minecraft("com.mojang:minecraft:1.21.1")
    mappings(loom.officialMojangMappings())

    modImplementation("net.fabricmc:fabric-loader:0.19.5")
    val fabricLanguageKotlin = "net.fabricmc:fabric-language-kotlin:1.13.11+kotlin.2.3.21"
    modImplementation(fabricLanguageKotlin)
    include(fabricLanguageKotlin)
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.11.0")

    api(project(":client-api"))
    implementation(project(":client-runtime"))
    implementation(project(":client-diagnostics"))
    implementation(project(":client-modules-performance"))
    implementation(project(":client-modules-accessibility"))
    implementation(project(":client-modules-qol"))

    include(project(":client-api"))
    include(project(":client-runtime"))
    include(project(":client-diagnostics"))
    include(project(":client-modules-performance"))
    include(project(":client-modules-accessibility"))
    include(project(":client-modules-qol"))
}

val modVersion = project.version.toString()

tasks.processResources {
    inputs.property("version", modVersion)
    filter<ReplaceTokens>("tokens" to mapOf("version" to modVersion))
}
