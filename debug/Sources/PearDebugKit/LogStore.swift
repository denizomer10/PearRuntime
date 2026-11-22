import Foundation

/// Owns the log directory for every launch PearSilicon makes. Each launch is a
/// session: the previous run is rotated to `<name>.1.log` (up to `keepRuns`)
/// instead of being truncated, so a crash log survives the relaunch that
/// usually follows it. Logs over `maximumBytes` are trimmed on rotation.
public struct LogStore: Sendable {
    public let directory: URL
    public var keepRuns: Int
    public var maximumBytes: Int64

    public init(directory: URL, keepRuns: Int = 4, maximumBytes: Int64 = 64 * 1_024 * 1_024) {
        self.directory = directory
        self.keepRuns = max(0, keepRuns)
        self.maximumBytes = maximumBytes
    }

    public func url(for appID: String, source: LogSource, run: Int = 0) -> URL {
        let suffix = run == 0 ? "" : ".\(run)"
        return directory.appendingPathComponent("\(appID)\(source.fileSuffix)\(suffix).log")
    }

    /// Rotates older runs and returns an empty, writable file for a new run.
    @discardableResult
    public func beginSession(appID: String, source: LogSource) throws -> URL {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let manager = FileManager.default
        let current = url(for: appID, source: source)
        if keepRuns > 0 {
            let oldest = url(for: appID, source: source, run: keepRuns)
            try? manager.removeItem(at: oldest)
            if keepRuns > 1 {
                for run in stride(from: keepRuns - 1, through: 1, by: -1) {
                    let from = url(for: appID, source: source, run: run)
                    if manager.fileExists(atPath: from.path) {
                        try? manager.moveItem(at: from, to: url(for: appID, source: source, run: run + 1))
                    }
                }
            }
            if manager.fileExists(atPath: current.path) {
                let previous = url(for: appID, source: source, run: 1)
                try? manager.moveItem(at: current, to: previous)
                trimIfNeeded(previous)
            }
        } else {
            try? manager.removeItem(at: current)
        }
        guard manager.createFile(atPath: current.path, contents: nil) else {
            throw CocoaError(.fileWriteUnknown, userInfo: [NSFilePathErrorKey: current.path])
        }
        return current
    }

    /// Opens a write handle for a new session, positioned at the start.
    public func beginSessionHandle(appID: String, source: LogSource) throws -> (URL, FileHandle) {
        let url = try beginSession(appID: appID, source: source)
        return (url, try FileHandle(forWritingTo: url))
    }

    /// All logs known for an app, newest run first within each source.
    public func logs(for appID: String) -> [(source: LogSource, run: Int, url: URL)] {
        var result: [(LogSource, Int, URL)] = []
        for source in LogSource.allCases {
            for run in 0...keepRuns {
                let url = url(for: appID, source: source, run: run)
                if FileManager.default.fileExists(atPath: url.path) { result.append((source, run, url)) }
            }
        }
        return result
    }

    /// Reads a log, keeping only the tail when it is larger than `limit` bytes.
    public static func read(_ url: URL, limit: Int = 32 * 1_024 * 1_024) throws -> String {
        let handle = try FileHandle(forReadingFrom: url)
        defer { try? handle.close() }
        let size = try handle.seekToEnd()
        if size > UInt64(limit) { try handle.seek(toOffset: size - UInt64(limit)) } else { try handle.seek(toOffset: 0) }
        let data = try handle.readToEnd() ?? Data()
        return String(decoding: data, as: UTF8.self)
    }

    private func trimIfNeeded(_ url: URL) {
        guard let size = (try? FileManager.default.attributesOfItem(atPath: url.path)[.size] as? NSNumber)?.int64Value,
              size > maximumBytes,
              let tail = try? Self.read(url, limit: Int(maximumBytes)) else { return }
        try? Data(("[PearDebug] log trimmed to the last \(maximumBytes) bytes\n" + tail).utf8)
            .write(to: url, options: .atomic)
    }
}
