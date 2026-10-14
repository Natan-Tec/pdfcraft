plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "com.natantec.pdfcraft"
    compileSdk = 34

    defaultConfig {
        applicationId = "com.natantec.pdfcraft"
        minSdk = 24
        targetSdk = 34
        versionCode = 1
        versionName = "0.8.0"
        ndk { abiFilters += listOf("arm64-v8a") }
    }

    buildTypes {
        // The Rust library is built in release mode either way; the debug key keeps the APK installable.
        getByName("release") {
            isMinifyEnabled = false
            signingConfig = signingConfigs.getByName("debug")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }

    packaging { jniLibs { useLegacyPackaging = true } }
}
