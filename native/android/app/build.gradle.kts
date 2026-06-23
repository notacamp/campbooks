import java.io.FileInputStream
import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

// Release signing is wired only when native/android/keystore.properties exists
// (it's git-ignored). Copy keystore.properties.example, fill it in, and create the
// upload keystore with keytool — see docs/app-store-deployment.md.
val keystorePropertiesFile = rootProject.file("keystore.properties")
val keystoreProperties = Properties().apply {
    if (keystorePropertiesFile.exists()) load(FileInputStream(keystorePropertiesFile))
}

android {
    namespace = "com.notacamp.campbooks"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.notacamp.campbooks"
        minSdk = 28          // Hotwire Native Android requires API 28+
        targetSdk = 35
        versionCode = 1
        versionName = "1.0"
    }

    signingConfigs {
        if (keystorePropertiesFile.exists()) {
            create("release") {
                storeFile = file(keystoreProperties.getProperty("storeFile"))
                storePassword = keystoreProperties.getProperty("storePassword")
                keyAlias = keystoreProperties.getProperty("keyAlias")
                keyPassword = keystoreProperties.getProperty("keyPassword")
            }
        }
    }

    buildTypes {
        release {
            // Signed only when keystore.properties is present; otherwise the build
            // is unsigned (local/CI can still assemble). Play upload needs a real key.
            if (keystorePropertiesFile.exists()) {
                signingConfig = signingConfigs.getByName("release")
            }
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
        }
    }

    buildFeatures {
        // BuildConfig.DEBUG gates dev-only logging in CampbooksApplication.
        // AGP 8 makes BuildConfig generation opt-in.
        buildConfig = true
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    packaging {
        resources {
            // Several transitive deps (okhttp, jspecify) ship identical OSGi metadata.
            excludes += "/META-INF/versions/9/OSGI-INF/MANIFEST.MF"
            excludes += "/META-INF/{AL2.0,LGPL2.1}"
        }
    }
}

kotlin {
    compilerOptions {
        jvmTarget = org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17
    }
}

dependencies {
    // Hotwire Native. Bump to the latest release (the two artifacts version
    // together): https://github.com/hotwired/hotwire-native-android/releases
    implementation("dev.hotwire:core:1.2.8")
    implementation("dev.hotwire:navigation-fragments:1.2.8")

    implementation("androidx.core:core-ktx:1.13.1")
    implementation("androidx.activity:activity-ktx:1.9.2")
    implementation("androidx.appcompat:appcompat:1.7.0")
    implementation("com.google.android.material:material:1.12.0")
    implementation("androidx.browser:browser:1.8.0")   // Chrome Custom Tabs
}
