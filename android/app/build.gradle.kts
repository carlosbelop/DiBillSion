import javax.inject.Inject

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
}

android {
    namespace = "com.dibillsion.app"
    compileSdk = 37

    defaultConfig {
        // Placeholder id: must be final before the first store upload (M8).
        applicationId = "com.dibillsion.app"
        minSdk = 26
        targetSdk = 37
        versionCode = 1
        versionName = "0.1.0"
    }

    buildTypes {
        debug {
            // A dev server on this computer, reached through `adb reverse tcp:8080 tcp:8080`,
            // unless `dibill.debugSyncUrl` points debug builds at a real server (the beta one).
            val syncUrl = providers.gradleProperty("dibill.debugSyncUrl").getOrElse("http://127.0.0.1:8080")
            buildConfigField("String", "SYNC_URL", "\"$syncUrl\"")
        }
        release {
            // The beta server, kept out of the public repo: Gradle property `dibill.syncUrl`
            // (e.g. in ~/.gradle/gradle.properties) or DIBILL_SYNC_URL. Empty: no server.
            val syncUrl = providers.gradleProperty("dibill.syncUrl")
                .orElse(providers.environmentVariable("DIBILL_SYNC_URL"))
                .getOrElse("")
            buildConfigField("String", "SYNC_URL", "\"$syncUrl\"")
            isMinifyEnabled = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
    }

    buildFeatures {
        compose = true
        buildConfig = true
    }
}

val repoRoot: File = rootDir.parentFile

/** Cross-compiles dibill-core for the app's ABIs with cargo-ndk. */
abstract class CargoNdkTask : DefaultTask() {
    @get:Inject abstract val exec: ExecOperations
    @get:Input abstract val profile: Property<String>
    @get:Internal abstract val workspace: DirectoryProperty
    @get:OutputDirectory abstract val outputDir: DirectoryProperty

    @TaskAction
    fun build() {
        exec.exec {
            workingDir = workspace.get().asFile
            commandLine(
                "cargo", "ndk",
                "-t", "arm64-v8a", "-t", "armeabi-v7a", "-t", "x86_64",
                "-o", outputDir.get().asFile.absolutePath,
                "build", "--profile", profile.get(), "-p", "dibill-core",
            )
        }
    }
}

/** Generates the Kotlin bindings for dibill-core. */
abstract class UniffiBindingsTask : DefaultTask() {
    @get:Inject abstract val exec: ExecOperations
    @get:Internal abstract val workspace: DirectoryProperty
    @get:OutputDirectory abstract val outputDir: DirectoryProperty

    @TaskAction
    fun generate() {
        exec.exec {
            workingDir = workspace.get().asFile
            commandLine("scripts/gen-bindings.sh", outputDir.get().asFile.absolutePath)
        }
    }
}

androidComponents {
    onVariants { variant ->
        val name = variant.name.replaceFirstChar { it.uppercase() }
        val cargo = tasks.register<CargoNdkTask>("cargoNdk$name") {
            profile.set(if (variant.buildType == "release") "mobile" else "dev")
            workspace.set(repoRoot)
            outputDir.set(layout.buildDirectory.dir("generated/rustJniLibs/${variant.name}"))
            // cargo does its own incremental build; let it decide.
            outputs.upToDateWhen { false }
        }
        val bindings = tasks.register<UniffiBindingsTask>("uniffiBindings$name") {
            workspace.set(repoRoot)
            outputDir.set(layout.buildDirectory.dir("generated/uniffi/${variant.name}"))
            outputs.upToDateWhen { false }
        }
        variant.sources.jniLibs?.addGeneratedSourceDirectory(cargo, CargoNdkTask::outputDir)
        // JVM unit tests load the host build of the core that the bindings task produces.
        tasks.matching { it.name == "test${name}UnitTest" }.configureEach {
            dependsOn(bindings)
            (this as Test).systemProperty("jna.library.path", File(repoRoot, "target/debug").absolutePath)
        }
        variant.sources.kotlin?.addGeneratedSourceDirectory(bindings, UniffiBindingsTask::outputDir)
    }
}

dependencies {
    implementation(platform(libs.compose.bom))
    implementation(libs.compose.material3)
    implementation(libs.compose.material.icons)
    implementation(libs.compose.ui)
    implementation(libs.compose.ui.tooling.preview)
    implementation(libs.activity.compose)
    implementation(libs.lifecycle.viewmodel.compose)
    implementation(libs.lifecycle.runtime.compose)
    implementation("${libs.jna.get()}@aar")
    debugImplementation(libs.compose.ui.tooling)
    testImplementation(libs.junit)
    testImplementation(libs.jna) // desktop JNA for the JVM smoke test
}
