// aiapp_host_bridge.h -- Mobile host bridge layer C header file
//
// Used by the iOS (Swift/ObjC) and Android (Kotlin + JNI) host App:
// the host implements callbacks (native capabilities) to drive the Rust
// runtime to play .aiapp application packages via the bridge layer.
//
// This header corresponds one-to-one with the #[no_mangle] exports in
// crates/aiapp-host-bridge/src/lib.rs.

#ifndef AIAPP_HOST_BRIDGE_H
#define AIAPP_HOST_BRIDGE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// ---- Host callback set (native capability implementations) ----

typedef struct AiappHostCallbacks AiappHostCallbacks;

struct AiappHostCallbacks {
    // Display a system notification (corresponds to WIT: show-notification)
    void (*show_notification)(const char* title, const char* body, void* ctx);

    // Save data (corresponds to WIT: save-data). Returns 0 on success.
    int (*save_data)(const char* key, const uint8_t* value, size_t len, void* ctx);

    // Load data (corresponds to WIT: load-data). Returns 0 on success and fills *out/*out_len;
    // the returned memory is allocated by this function and freed by the host via free_bytes.
    int (*load_data)(const char* key, uint8_t** out, size_t* out_len, void* ctx);

    // Free the memory returned by load_data / http_request / take_photo
    void (*free_bytes)(uint8_t* p, void* ctx);

    // In-app logging (corresponds to WIT: log)
    void (*log)(const char* level, const char* message, void* ctx);

    // Network request (corresponds to WIT: http-request). Returns 0 on success and fills *status/*out/*out_len.
    int (*http_request)(const char* url,
                        const char* method,
                        const char* const* headers,
                        size_t headers_len,
                        const uint8_t* body,
                        size_t body_len,
                        uint16_t* status,
                        uint8_t** out,
                        size_t* out_len,
                        void* ctx);

    // Get location (corresponds to WIT: get-location). Returns 0 on success and fills *lat/*lon.
    int (*get_location)(double* lat, double* lon, void* ctx);

    // Take photo / album (corresponds to WIT: take-photo). Returns 0 on success and fills *out/*out_len.
    int (*take_photo)(uint8_t** out, size_t* out_len, void* ctx);

    // Push token (corresponds to WIT: get-push-token). Returns 0 on success and fills *out (empty if none).
    int (*get_push_token)(char** out, void* ctx);

    // Free the C string returned by get_push_token
    void (*free_string)(char* s, void* ctx);

    // User context, passed as-is to all callbacks
    void* ctx;
};

// ---- Bridge layer API ----

// Create a host session and return a handle (returns NULL on failure). callbacks are
// shallow-copied; the host must ensure their lifetime covers the session.
AiappHostCallbacks* aiapp_bridge_create(const AiappHostCallbacks* callbacks);

// Parse the .aiapp package (pkg_path is the package directory). Returns 0 on success.
int aiapp_bridge_load(void* bridge, const char* pkg_path);

// Run the application. mode: "meta" (lightweight) | "wasmtime" (real WASM, requires feature).
// grant: comma-separated granted permissions (empty = all declared in the manifest). Returns 0 on success.
int aiapp_bridge_run(void* bridge, const char* mode, const char* grant);

// Get the most recent error message. Writes into buf (buf_len bytes) and returns the
// required length (including the trailing NUL).
size_t aiapp_bridge_last_error(const void* bridge, char* buf, size_t buf_len);

// Free the host session.
void aiapp_bridge_free(void* bridge);

#ifdef __cplusplus
}
#endif

#endif // AIAPP_HOST_BRIDGE_H
