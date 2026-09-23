pluginManagement {
    repositories {
        maven("https://maven.fabricmc.net")
        gradlePluginPortal()
        mavenCentral()
    }
}

dependencyResolutionManagement {
    // Loom supplies a local remapped-mod repository for exact Minecraft artifacts.
    repositoriesMode.set(RepositoriesMode.PREFER_PROJECT)
    repositories {
        maven("https://maven.fabricmc.net")
        mavenCentral()
    }
}

rootProject.name = "slate-client"

include(
    ":client-api",
    ":client-runtime",
    ":client-config",
    ":client-hud",
    ":client-diagnostics",
    ":client-modules-performance",
    ":client-modules-accessibility",
    ":client-modules-visual",
    ":client-modules-qol",
    ":client-modules-pvp",
    ":client-profiles",
    ":client-adapter-fabric",
    ":client-adapter-neoforge",
    ":client-benchmarks",
)

project(":client-api").projectDir = file("client/api")
project(":client-runtime").projectDir = file("client/runtime")
project(":client-config").projectDir = file("client/config")
project(":client-hud").projectDir = file("client/hud")
project(":client-diagnostics").projectDir = file("client/diagnostics")
project(":client-modules-performance").projectDir = file("client/modules/performance")
project(":client-modules-accessibility").projectDir = file("client/modules/accessibility")
project(":client-modules-visual").projectDir = file("client/modules/visual")
project(":client-modules-qol").projectDir = file("client/modules/qol")
project(":client-modules-pvp").projectDir = file("client/modules/pvp")
project(":client-profiles").projectDir = file("client/profiles")
project(":client-adapter-fabric").projectDir = file("client/adapters/fabric")
project(":client-adapter-neoforge").projectDir = file("client/adapters/neoforge")
project(":client-benchmarks").projectDir = file("client/benchmarks")
