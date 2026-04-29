plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("com.chaquo.python")
}

android {
    namespace = "io.hs.pgdb"
    compileSdk = 35

    defaultConfig {
        applicationId = "io.hs.pgdb"
        minSdk = 24
        targetSdk = 35
        versionCode = 2
        versionName = "0.1.1-poc"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"

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
}

chaquopy {
    defaultConfig {
        version = "3.11"

        pip {
            install("pydantic")
            install("fastapi")
            install("uvicorn")
            install("openai")
            install("httpx")
            install("h11")
            install("anyio")
            install("sniffio")
            install("typing-extensions")
            install("annotated-types")
            install("idna")
            install("certifi")
            install("tqdm")
        }

        extractPackages("pydantic_core")
    }
}

dependencies {
    implementation("androidx.core:core-ktx:1.15.0")
    implementation("androidx.appcompat:appcompat:1.7.0")
    implementation("com.google.android.material:material:1.12.0")
    implementation("androidx.constraintlayout:constraintlayout:2.2.1")
    implementation("com.squareup.okhttp3:okhttp:4.12.0")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.9.0")

    androidTestImplementation("androidx.test.ext:junit:1.2.1")
    androidTestImplementation("androidx.test:runner:1.6.2")
}
