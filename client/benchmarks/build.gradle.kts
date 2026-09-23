plugins {
    id("me.champeau.jmh")
}

dependencies {
    implementation(project(":client-config"))
    implementation(project(":client-hud"))
    implementation(project(":client-modules-performance"))
}

jmh {
    benchmarkMode = listOf("avgt")
    fork = 1
    iterations = 3
    timeOnIteration = "1s"
    warmupIterations = 2
    warmup = "1s"
    jmhVersion = "1.37"
    resultFormat = "JSON"
}
