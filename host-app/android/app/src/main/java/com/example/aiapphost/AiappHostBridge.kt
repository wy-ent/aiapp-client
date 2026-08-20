// AiappHostBridge.kt -- Android host App bridge-layer wrapper
//
// Calls the Rust cdylib (libaiapp_host_bridge.so) via JNI to implement WIT host capabilities:
// storage (app sandbox directory) / notifications (NotificationManager) / network (OkHttp or HttpURLConnection)
// / location (FusedLocationProvider) / camera (CameraX or Intent) / push (FCM).
//
// Build (requires Android NDK + cargo-ndk):
//   cargo install cargo-ndk
//   cd aiapp-mb && cargo ndk -t arm64-v8a -o host-app/android/app/src/main/jniLibs build -p aiapp-host-bridge

package com.example.aiapphost

import android.content.Context
import java.io.File

/**
 * Context for native capability callbacks (data directory + Android Context).
 */
class HostContext(val appContext: Context, val dataDir: File)

/**
 * Wraps the aiapp_host_bridge C-ABI. All methods must be called off the main thread
 * (the bridge layer runs an internal tokio runtime).
 */
class AiappHostBridge(private val ctx: HostContext) {

    // JNI declarations (correspond to #[no_mangle] in lib.rs)
    private external fun nativeCreate(callbacks: Long): Long
    private external fun nativeLoad(handle: Long, pkgPath: String): Int
    private external fun nativeRun(handle: Long, mode: String, grant: String): Int
    private external fun nativeLastError(handle: Long, buf: ByteArray, len: Int): Int
    private external fun nativeFree(handle: Long)

    // Callback context address (stores the callback struct pointer as a Long)
    private var handle: Long = 0

    init {
        System.loadLibrary("aiapp_host_bridge")
    }

    /** Create a host session. callbacksPtr is the AiappHostCallbacks struct pointer built on the JNI side. */
    fun create(callbacksPtr: Long): Boolean {
        handle = nativeCreate(callbacksPtr)
        return handle != 0L
    }

    /** Load the .aiapp package directory. */
    fun load(packagePath: String) {
        check(handle != 0L) { "host session not created" }
        val rc = nativeLoad(handle, packagePath)
        check(rc == 0) { "load failed: $lastError" }
    }

    /** Run the application (mode: meta / wasmtime). */
    fun run(mode: String = "meta", grant: String = "") {
        check(handle != 0L) { "host session not created" }
        val rc = nativeRun(handle, mode, grant)
        check(rc == 0) { "run failed: $lastError" }
    }

    val lastError: String
        get() {
            if (handle == 0L) return "handle is null"
            val n = nativeLastError(handle, ByteArray(0), 0)
            if (n <= 0) return ""
            val buf = ByteArray(n)
            nativeLastError(handle, buf, n)
            return buf.toString(Charsets.UTF_8).trimEnd('\u0000')
        }

    fun release() {
        if (handle != 0L) {
            nativeFree(handle)
            handle = 0L
        }
    }

    /**
     * Build the host callback struct (the JNI C layer allocates AiappHostCallbacks and fills in the function pointers).
     * The Kotlin side registers static callbacks via JNI here.
     */
    fun registerNativeCallbacks(): Long {
        // Done by the JNI bridge (see JniHostCallbacks.c / initialized after System.loadLibrary in MainActivity):
        // builds the callback struct and returns the pointer. The Kotlin side registers via JNI static methods.
        return initNativeCallbacks(this)
    }

    private external fun initNativeCallbacks(bridge: AiappHostBridge): Long

    // ---- Native capability implementations (called by JNI callbacks) ----

    /** Save data to the app sandbox dataDir. */
    fun onSaveData(key: String, value: ByteArray): Boolean = runCatching {
        val file = File(ctx.dataDir, key)
        file.parentFile?.mkdirs()
        file.writeBytes(value)
    }.isSuccess

    /** Load data. */
    fun onLoadData(key: String): ByteArray? =
        File(ctx.dataDir, key).takeIf { it.exists() }?.readBytes()

    /** Send a system notification. */
    fun onNotify(title: String, body: String) {
        // Integrate NotificationManagerCompat + notification channel
        android.util.Log.i("aiapp-host", "notify: $title - $body")
    }

    /** Print a log message. */
    fun onLog(level: String, message: String) {
        android.util.Log.println(levelToPrio(level), "aiapp-host", message)
    }

    private fun levelToPrio(level: String): Int = when (level) {
        "error" -> android.util.Log.ERROR
        "warn" -> android.util.Log.WARN
        else -> android.util.Log.INFO
    }

    /** Network request (can be swapped for OkHttp). */
    fun onHttpRequest(
        url: String,
        method: String,
        headers: List<String>,
        body: ByteArray?,
    ): Pair<Int, ByteArray> {
        val conn = (java.net.URL(url).openConnection() as java.net.HttpURLConnection).apply {
            requestMethod = method
            headers.forEach { h ->
                val i = h.indexOf(':')
                if (i > 0) setRequestProperty(h.substring(0, i).trim(), h.substring(i + 1).trim())
            }
            if (body != null) {
                doOutput = true
                outputStream.use { it.write(body) }
            }
        }
        val code = conn.responseCode
        val data = (if (code in 200..299) conn.inputStream else conn.errorStream)
            ?.use { it.readBytes() } ?: ByteArray(0)
        return code to data
    }
}
