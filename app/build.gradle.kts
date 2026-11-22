plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

// Native core (Rust). cargo-ndk writes libstroid_jni.so into this directory,
// laid out as <dir>/<abi>/libstroid_jni.so, and it is packaged as jniLibs.
val rustJniLibs = layout.buildDirectory.dir("rustJniLibs").get().asFile

val cargoNdkBuild = tasks.register<Exec>("cargoNdkBuild") {
    group = "rust"
    description = "Builds libstroid_jni.so for arm64-v8a with cargo-ndk."
    workingDir = rootProject.file("core")
    commandLine(
        "cargo", "ndk",
        "-t", "arm64-v8a",
        "-o", rustJniLibs.absolutePath,
        "build", "--release", "-p", "stroid-jni",
    )
    // Pass -PskipCargo to reuse already-built native libraries.
    onlyIf { !project.hasProperty("skipCargo") }
}

android {
    namespace = "dev.stroid.app"
    compileSdk = 35

    defaultConfig {
        applicationId = "dev.stroid.app"
        minSdk = 26

        // Intentionally 28, not the latest. Android 10+ (targetSdk >= 29)
        // forbids executing files from the app's data directory, which a
        // real terminal and installable toolchains depend on. This is the
        // same approach Termux uses. The app is therefore distributed outside
        // Google Play (GitHub Releases / F-Droid). See docs/bootstrap.md.
        @Suppress("ExpiredTargetSdkVersion")
        targetSdk = 28

        versionCode = 1
        versionName = "0.1.0"

        ndk {
            abiFilters += "arm64-v8a"
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }

    buildFeatures {
        compose = true
    }

    packaging {
        jniLibs {
            // Keep native libraries extracted on disk (needed later to run
            // bundled executables shipped as lib*.so).
            useLegacyPackaging = true
        }
    }

    lint {
        // targetSdk 28 is deliberate (see above).
        disable += "ExpiredTargetSdkVersion"
    }

    sourceSets {
        getByName("main") {
            jniLibs.srcDir(rustJniLibs)
        }
    }
}

tasks.matching { it.name == "preBuild" }.configureEach {
    dependsOn(cargoNdkBuild)
}

dependencies {
    implementation("androidx.core:core-ktx:1.13.1")
    implementation("androidx.activity:activity-compose:1.9.3")

    implementation(platform("androidx.compose:compose-bom:2024.10.01"))
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.material3:material3")
}
