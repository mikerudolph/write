plugins {
    id("com.android.application")
}

android {
    namespace = "com.mikerudolph.write"
    compileSdk = 36

    defaultConfig {
        applicationId = "com.mikerudolph.write"
        minSdk = 29
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"
        ndk {
            abiFilters += listOf("arm64-v8a", "x86_64")
        }
    }

    sourceSets["main"].kotlin.srcDir("build/generated/uniffi")
}

dependencies {
    implementation("net.java.dev.jna:jna:5.19.1@aar")
}
