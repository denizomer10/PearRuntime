import Darwin
import Foundation

/// Finds the processes of a Wine prefix, whichever engine (MetalSharp, WineHQ…)
/// started them: programs a launcher re-executed and orphans included. Wine
/// ships no process list, and its processes rewrite their names, so they are
/// matched by working directory instead:
///
/// - Windows processes run inside the game's folder (its `Prefix/drive_c/…`
///   or `Payload/…`);
/// - the prefix's wineserver runs in `/tmp/.wine-<uid>/server-<dev>-<inode>`,
///   named after the prefix folder's device and inode.
///
/// Only processes of the current user are inspected.
public enum WineProcesses {
    public struct Member: Codable, Equatable, Sendable {
        public let pid: Int32
        /// Lowercased Windows file name (`steam.exe`) or Unix name (`wineserver`).
        public let name: String
        /// Working directory, for reports.
        public let directory: String
        public let isServer: Bool
        /// Wine's own background services, which do not mean a program is open.
        public var isWineService: Bool { isServer || WineProcesses.wineServices.contains(name) }

        public init(pid: Int32, name: String, directory: String = "", isServer: Bool) {
            self.pid = pid
            self.name = name
            self.directory = directory
            self.isServer = isServer
        }
    }

    /// A folder to match against: `root` contains everything the program runs
    /// from (for PearSilicon, the entry folder), `prefix` is its WINEPREFIX.
    public struct Target: Sendable {
        public let id: String
        public let root: URL
        public let prefix: URL
        public init(id: String, root: URL, prefix: URL) {
            self.id = id
            self.root = root
            self.prefix = prefix
        }
    }

    public static let wineServices: Set<String> = [
        "services.exe", "winedevice.exe", "plugplay.exe", "svchost.exe", "explorer.exe", "rpcss.exe",
        "conhost.exe", "wineboot.exe", "winemenubuilder.exe", "start.exe", "tabtip.exe", "rundll32.exe",
        "mscorsvw.exe", "wineserver", "wine-preloader", "wine64-preloader",
    ]

    /// Processes of each target, keyed by target ID. One pass over the process table.
    public static func scan(_ targets: [Target]) -> [String: [Member]] {
        let prepared = targets.map { (id: $0.id, root: canonical($0.root.path) + "/", server: serverDirectory(for: $0.prefix)) }
        guard !prepared.isEmpty else { return [:] }
        var result: [String: [Member]] = [:]
        for pid in userPIDs() where pid != getpid() {
            guard let cwd = workingDirectory(pid) else { continue }
            for target in prepared {
                let isServer = target.server.map { cwd == $0 } ?? false
                guard isServer || cwd.hasPrefix(target.root) else { continue }
                result[target.id, default: []].append(
                    Member(pid: pid, name: processName(pid) ?? "", directory: cwd, isServer: isServer))
                break
            }
        }
        return result
    }

    /// Whether a Windows program (not only Wine's services) is running.
    public static func hasProgram(_ members: [Member]?) -> Bool {
        members?.contains { !$0.isWineService } ?? false
    }

    /// SIGKILL to every member still alive, repeated until none is left or the
    /// timeout passes. Returns what could not be stopped.
    public static func kill(_ target: Target, timeout: Duration = .seconds(5)) async -> [Member] {
        let deadline = ContinuousClock.now.advanced(by: timeout)
        while true {
            let left = scan([target])[target.id] ?? []
            if left.isEmpty || ContinuousClock.now >= deadline { return left }
            for member in left { Darwin.kill(member.pid, SIGKILL) }
            try? await Task.sleep(for: .milliseconds(100))
        }
    }

    /// `/private/tmp/.wine-<uid>/server-<dev hex>-<inode hex>` for a prefix.
    public static func serverDirectory(for prefix: URL) -> String? {
        var info = stat()
        guard stat(prefix.path, &info) == 0 else { return nil }
        return "/private/tmp/.wine-\(getuid())/server-\(String(UInt64(UInt32(bitPattern: info.st_dev)), radix: 16))-\(String(info.st_ino, radix: 16))"
    }

    // MARK: - Process table

    static func canonical(_ path: String) -> String {
        guard let resolved = realpath(path, nil) else { return path }
        defer { free(resolved) }
        return String(cString: resolved)
    }

    static func userPIDs() -> [pid_t] {
        var mib: [Int32] = [CTL_KERN, KERN_PROC, KERN_PROC_UID, Int32(bitPattern: getuid())]
        var size = 0
        guard sysctl(&mib, UInt32(mib.count), nil, &size, nil, 0) == 0, size > 0 else { return [] }
        // Room for processes started between the two calls.
        size += 64 * MemoryLayout<kinfo_proc>.stride
        var processes = [kinfo_proc](repeating: kinfo_proc(), count: size / MemoryLayout<kinfo_proc>.stride)
        guard sysctl(&mib, UInt32(mib.count), &processes, &size, nil, 0) == 0 else { return [] }
        return processes.prefix(size / MemoryLayout<kinfo_proc>.stride).map { $0.kp_proc.p_pid }.filter { $0 > 0 }
    }

    static func workingDirectory(_ pid: pid_t) -> String? {
        var info = proc_vnodepathinfo()
        let size = Int32(MemoryLayout<proc_vnodepathinfo>.size)
        guard proc_pidinfo(pid, PROC_PIDVNODEPATHINFO, 0, &info, size) == size else { return nil }
        return withUnsafeBytes(of: info.pvi_cdir.vip_path) { raw in
            let bytes = raw.bindMemory(to: CChar.self)
            guard let base = bytes.baseAddress, base.pointee != 0 else { return nil }
            return String(cString: base)
        }
    }

    /// Wine sets argv[0] to the Windows path (`C:\windows\system32\services.exe`).
    static func processName(_ pid: pid_t) -> String? {
        var mib: [Int32] = [CTL_KERN, KERN_PROCARGS2, pid]
        var size = 0
        guard sysctl(&mib, 3, nil, &size, nil, 0) == 0, size > MemoryLayout<Int32>.size else { return nil }
        var buffer = [UInt8](repeating: 0, count: size)
        guard sysctl(&mib, 3, &buffer, &size, nil, 0) == 0 else { return nil }
        // argc, executable path, NUL padding, then argv[0].
        var index = MemoryLayout<Int32>.size
        while index < size, buffer[index] != 0 { index += 1 }
        while index < size, buffer[index] == 0 { index += 1 }
        guard index < size else { return nil }
        let end = buffer[index..<size].firstIndex(of: 0) ?? size
        let argv0 = String(decoding: buffer[index..<end], as: UTF8.self)
        let last = argv0.split(whereSeparator: { $0 == "\\" || $0 == "/" }).last.map(String.init) ?? argv0
        return last.lowercased()
    }
}
