import java.util.Properties

// Path del keystore local (generado por scripts/keystore.sh; CI lo deposita)
val keystorePropsFile = rootProject.file("local/keystore/keystore.properties")
val ksProps = Properties().apply {
    if (keystorePropsFile.exists()) keystorePropsFile.inputStream().use { load(it) }
}

fun gitCommitCount(): Int = runCatching {
    val p = ProcessBuilder("git", "rev-list", "--count", "HEAD")
        .directory(rootProject.projectDir)
        .redirectErrorStream(true)
        .start()
    val out = p.inputStream.readBytes().decodeToString().trim().toInt()
    p.waitFor()
    out
}.getOrDefault(1)

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "com.ruuuberth.xclient"
    compileSdk = 37

    defaultConfig {
        applicationId = "com.ruuuberth.xclient"
        minSdk = 26
        targetSdk = 36
        versionCode = gitCommitCount()
        versionName = "0.1.$versionCode"
    }

    if (keystorePropsFile.exists()) {
        signingConfigs {
            create("shared") {
                storeFile = file(ksProps.getProperty("storeFile"))
                storePassword = ksProps.getProperty("storePassword")
                keyAlias = ksProps.getProperty("keyAlias")
                keyPassword = ksProps.getProperty("keyPassword")
            }
        }
    }

    buildTypes {
        getByName("debug") {
            if (keystorePropsFile.exists()) {
                signingConfig = signingConfigs.getByName("shared")
            }
        }
        getByName("release") {
            isMinifyEnabled = false
            if (keystorePropsFile.exists()) {
                signingConfig = signingConfigs.getByName("shared")
            }
        }
    }

    buildFeatures {
        compose = true
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

dependencies {
    implementation(platform("androidx.compose:compose-bom:2026.09.00"))
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.activity:activity-compose:1.13.0")
    implementation("androidx.core:core-ktx:1.19.1")
}
