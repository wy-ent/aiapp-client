//
//  AiappHostBridge.swift
//  aiapp-ios-host
//
//  iOS host App: bridge-layer wrapper.
//  - Implements the callbacks of aiapp_host_bridge.h (native capabilities: notification / file storage / network / location / camera / push)
//  - Loads the .aiapp package and drives the Rust runtime to play the application
//
//  Build: link libaiapp_host_bridge.dylib into the project and enable the
//  Module Map or import the header directly within the target.

import Foundation

/// Bridge error
enum AiappHostError: Error, CustomStringConvertible {
    case nullHandle
    case loadFailed(String)
    case runFailed(String)

    var description: String {
        switch self {
        case .nullHandle: return "host session handle is null"
        case .loadFailed(let m): return "load failed: \(m)"
        case .runFailed(let m): return "run failed: \(m)"
        }
    }
}

/// Wraps the native capability callbacks (C function pointer context)
final class HostCallbackContext {
    /// Data directory (corresponds to WIT save-data/load-data -> local file)
    let dataDir: URL
    init(dataDir: URL) { self.dataDir = dataDir }
}

/// iOS host: wraps the aiapp_host_bridge C-ABI
final class AiappHostBridge {
    private var handle: OpaquePointer?

    /// Create a host session
    init(dataDir: URL) throws {
        let ctx = Unmanaged.passRetained(HostCallbackContext(dataDir: dataDir)).toOpaque()

        var cb = AiappHostCallbacks()
        cb.ctx = ctx
        cb.show_notification = { title, body, _ in
            let t = String(cString: title!)
            let b = String(cString: body!)
            DispatchQueue.main.async {
                // Integrate UNUserNotificationCenter to send the system notification
                print("[aiapp:notification] \(t): \(b)")
            }
        }
        cb.save_data = { key, value, len, c in
            guard let c else { return -1 }
            let hostCtx = Unmanaged<HostCallbackContext>.fromOpaque(c).takeUnretainedValue()
            let url = hostCtx.dataDir.appendingPathComponent(String(cString: key!))
            do {
                try FileManager.default.createDirectory(
                    at: url.deletingLastPathComponent(),
                    withIntermediateDirectories: true)
                try Data(bytes: value!, count: len).write(to: url)
                return 0
            } catch {
                return -1
            }
        }
        cb.load_data = { key, out, outLen, c in
            guard let c else { return -1 }
            let hostCtx = Unmanaged<HostCallbackContext>.fromOpaque(c).takeUnretainedValue()
            let url = hostCtx.dataDir.appendingPathComponent(String(cString: key!))
            guard let data = try? Data(contentsOf: url) else { return -1 }
            let buf = data.withUnsafeBytes { (raw: UnsafeRawBufferPointer) -> UnsafeMutablePointer<UInt8> in
                let p = UnsafeMutablePointer<UInt8>.allocate(capacity: data.count)
                p.initialize(from: raw.bindMemory(to: UInt8.self).baseAddress!, count: data.count)
                return p
            }
            out?.pointee = buf
            outLen?.pointee = data.count
            return 0
        }
        cb.free_bytes = { p, _ in
            p?.deallocate()
        }
        cb.log = { level, message, _ in
            print("[aiapp:\(String(cString: level!))] \(String(cString: message!))")
        }
        cb.http_request = { url, method, headers, headersLen, body, bodyLen, status, out, outLen, c in
            guard let c else { return -1 }
            var req = URLRequest(url: URL(string: String(cString: url!))!)
            req.httpMethod = String(cString: method!)
            if let headers, headersLen > 0 {
                for i in 0..<headersLen {
                    let pair = String(cString: headers[Int(i)]).split(separator: ":", maxSplits: 1)
                    if pair.count == 2 {
                        req.setValue(String(pair[1]).trimmingCharacters(in: .whitespaces),
                                     forHTTPHeaderField: String(pair[0]).trimmingCharacters(in: .whitespaces))
                    }
                }
            }
            if let body, bodyLen > 0 {
                req.httpBody = Data(bytes: body, count: bodyLen)
            }
            // Execute the network request synchronously (use a semaphore to wait when the host thread model allows)
            let sem = DispatchSemaphore(value: 0)
            var respData: Data?
            var respStatus: Int = 0
            var failed = false
            URLSession.shared.dataTask(with: req) { data, resp, _ in
                respData = data
                respStatus = (resp as? HTTPURLResponse)?.statusCode ?? 0
                sem.signal()
            }.resume()
            _ = sem.wait(timeout: .now() + 30)
            if failed { return -1 }
            status?.pointee = UInt16(respStatus)
            if let respData, respData.count > 0 {
                let buf = UnsafeMutablePointer<UInt8>.allocate(capacity: respData.count)
                respData.withUnsafeBytes { raw in
                    buf.initialize(from: raw.bindMemory(to: UInt8.self).baseAddress!, count: respData.count)
                }
                out?.pointee = buf
                outLen?.pointee = respData.count
            }
            return 0
        }
        cb.get_location = { lat, lon, _ in
            // Integrate CLLocationManager to obtain the location
            lat?.pointee = 0.0
            lon?.pointee = 0.0
            return -1 // Location permission not yet integrated
        }
        cb.take_photo = { _, _, _ in -1 } // Integrate UIImagePickerController
        cb.get_push_token = { _, _ in -1 } // Integrate APNs

        guard let handle = aiapp_bridge_create(&cb) else {
            throw AiappHostError.nullHandle
        }
        self.handle = OpaquePointer(handle)
    }

    /// Load the .aiapp package (package directory path)
    func load(packagePath: String) throws {
        guard let handle else { throw AiappHostError.nullHandle }
        let rc = aiapp_bridge_load(handle, packagePath)
        if rc != 0 { throw AiappHostError.loadFailed(lastError) }
    }

    /// Run the application
    func run(mode: String = "meta", grant: String = "") throws {
        guard let handle else { throw AiappHostError.nullHandle }
        let rc = aiapp_bridge_run(handle, mode, grant)
        if rc != 0 { throw AiappHostError.runFailed(lastError) }
    }

    /// Most recent error
    var lastError: String {
        guard let handle else { return "handle is null" }
        let n = aiapp_bridge_last_error(handle, nil, 0)
        var buf = [CChar](repeating: 0, count: n)
        _ = aiapp_bridge_last_error(handle, &buf, n)
        return String(cString: buf)
    }

    deinit {
        if let handle {
            aiapp_bridge_free(handle)
            self.handle = nil
        }
    }
}
