// Minimal Unix-domain socket server for the IMK helper.
//
// One thread accepts, one thread per accepted connection reads one
// newline-delimited JSON request, dispatches to the supplied handler,
// writes back one newline-delimited JSON response. Connection closes
// after each request — matches the short-lived round-trip style of the
// Rust client. Concurrent clients are fine but not the design point.

import Foundation

public enum SocketError: Error, Equatable {
    case bindFailed(path: String, errno: Int32)
    case listenFailed(errno: Int32)
    case notRunning
}

public final class SocketServer {
    /// Filesystem path the server listens on. Default matches the Rust
    /// client's SOCKET_PATH constant; tests override.
    public let path: String

    private var listenFd: Int32 = -1
    private var accepting: Bool = false
    private let acceptQueue: DispatchQueue
    private let workQueue: DispatchQueue
    private let handler: (ImkRequest) -> ImkResponse

    public init(
        path: String = "/tmp/audio-input-imk.sock",
        handler: @escaping (ImkRequest) -> ImkResponse
    ) {
        self.path = path
        self.handler = handler
        self.acceptQueue = DispatchQueue(label: "imk-helper.accept")
        self.workQueue = DispatchQueue(label: "imk-helper.work", attributes: .concurrent)
    }

    /// Bind + listen. Throws if the path is in use or the syscall fails.
    /// Idempotent unlink of the existing socket file beforehand — if a
    /// previous helper crashed without cleaning up, we replace its socket.
    public func start() throws {
        // Stale socket from a crashed previous run? Remove it. Ignore
        // failures here — if it's not a stale socket, the bind below
        // will fail with a useful errno.
        unlink(path)

        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else {
            throw SocketError.bindFailed(path: path, errno: errno)
        }

        // Fill sockaddr_un with the path. Note sun_path has a fixed size
        // (104 bytes on macOS) so paths longer than that fail loudly here.
        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        let pathBytes = path.utf8CString
        let pathCap = MemoryLayout.size(ofValue: addr.sun_path)
        guard pathBytes.count <= pathCap else {
            close(fd)
            throw SocketError.bindFailed(path: path, errno: ENAMETOOLONG)
        }
        withUnsafeMutablePointer(to: &addr.sun_path) { p in
            p.withMemoryRebound(to: CChar.self, capacity: pathCap) { dst in
                pathBytes.withUnsafeBufferPointer { src in
                    _ = memcpy(dst, src.baseAddress!, pathBytes.count)
                }
            }
        }

        let addrSize = socklen_t(MemoryLayout<sockaddr_un>.size)
        let bindResult = withUnsafePointer(to: &addr) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { sa in
                Darwin.bind(fd, sa, addrSize)
            }
        }
        guard bindResult == 0 else {
            let e = errno
            close(fd)
            throw SocketError.bindFailed(path: path, errno: e)
        }

        guard Darwin.listen(fd, 16) == 0 else {
            let e = errno
            close(fd)
            throw SocketError.listenFailed(errno: e)
        }

        listenFd = fd
        accepting = true
        acceptLoop()
    }

    /// Stop accepting + close the listen socket + unlink the path.
    /// Idempotent — safe to call repeatedly or before `start()`.
    public func stop() {
        accepting = false
        if listenFd >= 0 {
            close(listenFd)
            listenFd = -1
        }
        unlink(path)
    }

    deinit { stop() }

    private func acceptLoop() {
        let fd = listenFd
        let h = handler
        let work = workQueue
        acceptQueue.async { [weak self] in
            while self?.accepting == true {
                var clientAddr = sockaddr()
                var len = socklen_t(MemoryLayout<sockaddr>.size)
                let client = Darwin.accept(fd, &clientAddr, &len)
                if client < 0 {
                    // Listening socket got closed during stop() — exit cleanly.
                    if errno == EBADF || errno == EINVAL { return }
                    continue
                }
                work.async { Self.handleClient(client, h) }
            }
        }
    }

    private static func handleClient(_ fd: Int32, _ handler: (ImkRequest) -> ImkResponse) {
        defer { close(fd) }
        guard let line = readLine(fd: fd) else { return }
        let resp: ImkResponse
        do {
            let req = try JSONDecoder().decode(ImkRequest.self, from: Data(line.utf8))
            resp = handler(req)
        } catch {
            resp = .error(message: "decode failed: \(error)")
        }
        guard var out = try? JSONEncoder().encode(resp) else { return }
        out.append(UInt8(ascii: "\n"))
        out.withUnsafeBytes { buf in
            _ = write(fd, buf.baseAddress, buf.count)
        }
    }

    /// Read up to the first newline. Returns nil if the connection closes
    /// before we see one or if the request is unreasonably large (16 KiB cap).
    private static func readLine(fd: Int32) -> String? {
        var buf = [UInt8](repeating: 0, count: 4096)
        var collected = [UInt8]()
        let maxBytes = 16 * 1024
        while collected.count < maxBytes {
            let n = read(fd, &buf, buf.count)
            if n <= 0 { return nil }
            for i in 0..<n {
                let b = buf[i]
                if b == UInt8(ascii: "\n") {
                    collected.append(contentsOf: buf[0..<i])
                    return String(bytes: collected, encoding: .utf8)
                }
            }
            collected.append(contentsOf: buf[0..<n])
        }
        return nil
    }
}
