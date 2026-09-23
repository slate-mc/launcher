plugins {
    `java-library`
}

dependencies {
    api(project(":client-api"))
    implementation(project(":client-runtime"))
}
