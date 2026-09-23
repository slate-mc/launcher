import com.diffplug.gradle.spotless.SpotlessExtension
import org.gradle.api.tasks.compile.JavaCompile
import org.jetbrains.kotlin.gradle.dsl.JvmTarget
import org.jetbrains.kotlin.gradle.dsl.KotlinJvmProjectExtension
import org.jetbrains.kotlin.gradle.tasks.KotlinCompile

plugins {
    kotlin("jvm") version "2.3.21" apply false
    id("com.diffplug.spotless") version "8.10.2"
    id("dev.detekt") version "2.0.0-alpha.3" apply false
    id("me.champeau.jmh") version "0.7.3" apply false
}

group = "org.slatelauncher.client"
version = "0.1.0-SNAPSHOT"

spotless {
    kotlinGradle {
        target("build.gradle.kts", "settings.gradle.kts")
        ktlint("1.8.0")
        trimTrailingWhitespace()
        endWithNewline()
    }
}

subprojects {
    group = rootProject.group
    version = rootProject.version

    apply(plugin = "org.jetbrains.kotlin.jvm")
    apply(plugin = "dev.detekt")
    apply(plugin = "com.diffplug.spotless")

    extensions.configure<SpotlessExtension> {
        kotlin {
            target("src/**/*.kt")
            ktlint("1.8.0")
            trimTrailingWhitespace()
            endWithNewline()
        }
        kotlinGradle {
            target("*.gradle.kts")
            ktlint("1.8.0")
            trimTrailingWhitespace()
            endWithNewline()
        }
        java {
            target("src/**/*.java")
            googleJavaFormat("1.30.0")
            removeUnusedImports()
            forbidWildcardImports()
            endWithNewline()
        }
    }

    extensions.configure<KotlinJvmProjectExtension> {
        explicitApi()
        jvmToolchain(25)
    }

    tasks.withType<KotlinCompile>().configureEach {
        compilerOptions {
            allWarningsAsErrors.set(true)
            jvmTarget.set(JvmTarget.JVM_21)
            progressiveMode.set(true)
            freeCompilerArgs.add("-Xjsr305=strict")
        }
    }

    tasks.withType<JavaCompile>().configureEach {
        options.release.set(21)
    }

    tasks.withType<Test>().configureEach {
        useJUnitPlatform()
        testLogging {
            events("failed", "skipped")
            exceptionFormat = org.gradle.api.tasks.testing.logging.TestExceptionFormat.FULL
        }
    }

    dependencies {
        "testImplementation"(platform("org.junit:junit-bom:5.14.3"))
        "testImplementation"("org.junit.jupiter:junit-jupiter")
        "testRuntimeOnly"("org.junit.platform:junit-platform-launcher")
    }

    dependencyLocking {
        lockAllConfigurations()
    }

    tasks.named("check") {
        dependsOn("detekt")
    }
}

tasks.register("clientCheck") {
    group = "verification"
    description = "Runs all Slate Client formatting, analysis, tests, and benchmark compilation."
    dependsOn("spotlessCheck")
    dependsOn(subprojects.map { "${it.path}:check" })
    dependsOn(subprojects.map { "${it.path}:spotlessCheck" })
    dependsOn(":client-benchmarks:jmhClasses")
}
