plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
}

android {
    namespace = "app.quire.notes"
    compileSdk = 36

    defaultConfig {
        applicationId = "app.quire.notes"
        minSdk = 26
        targetSdk = 36
        versionCode = 11
        versionName = "0.5.6"
    }

    // Release signing reads its key from Gradle properties, for example in
    // ~/.gradle/gradle.properties, so no key or password is in this repository.
    val storeFile = providers.gradleProperty("QUIRE_RELEASE_STORE_FILE").orNull
    if (storeFile != null) {
        signingConfigs {
            create("release") {
                this.storeFile = file(storeFile)
                storePassword = providers.gradleProperty("QUIRE_RELEASE_STORE_PASSWORD").get()
                keyAlias = providers.gradleProperty("QUIRE_RELEASE_KEY_ALIAS").get()
                keyPassword = providers.gradleProperty("QUIRE_RELEASE_KEY_PASSWORD").get()
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            signingConfig = signingConfigs.findByName("release")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    buildFeatures {
        compose = true
        // The update check reads VERSION_NAME and DEBUG.
        buildConfig = true
    }

    packaging {
        // This prebuilt library ships without debug symbols. Skip the strip step, which needs the NDK.
        jniLibs.keepDebugSymbols += "**/libandroidx.graphics.path.so"
    }
}

dependencies {
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.documentfile)
    implementation(platform(libs.compose.bom))
    implementation(libs.compose.ui)
    implementation(libs.compose.foundation)
    implementation(libs.compose.material3)
    testImplementation(libs.junit)
}
