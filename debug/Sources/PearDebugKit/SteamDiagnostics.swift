import Foundation

/// Explains the state of a Windows Steam client running under Wine from its
/// own logs (`<Steam>/logs`) and the prefix registry (`user.reg`). Each check
/// comes from a failure seen in practice:
///
/// - MetalSharp's kernelbase launcher rule matches `\Steam\bin\cef\` and makes
///   the CEF StorageService fail (`target_services.cc`, `InitCalled`);
/// - Steam saves the folder it was started from as its install folder; a drive
///   root there triggers "install folder is not writable… Repair" on the next
///   start, and Steam closes its UI while that dialog waits.
public struct SteamDiagnostics: Sendable {
    public let steamFolder: URL

    public init(steamFolder: URL) {
        self.steamFolder = steamFolder
    }

    /// The prefix that holds `drive_c/…/Steam`, if the folder is inside one.
    public var prefix: URL? {
        var current = steamFolder.standardizedFileURL
        while current.path != "/" {
            if current.lastPathComponent == "drive_c" { return current.deletingLastPathComponent() }
            current = current.deletingLastPathComponent()
        }
        return nil
    }

    public func findings() -> [Finding] {
        let logs = steamFolder.appendingPathComponent("logs", isDirectory: true)
        guard FileManager.default.fileExists(atPath: logs.path) else {
            return [Finding(id: "steam.no-logs", severity: .warning, title: "No Steam logs found",
                            detail: "\(logs.path) does not exist.",
                            suggestion: "Pass the Steam client folder (the one that contains steam.exe and logs).")]
        }
        var findings: [Finding] = []
        let console = read(logs, "console_log.txt")
        if let build = lastMatch(in: console, after: "Client version: ") {
            findings.append(Finding(id: "steam.build", severity: .info, title: "Steam client build \(build)", detail: ""))
        }

        // Install path saved in the registry.
        if let prefix, let registry = try? String(contentsOf: prefix.appendingPathComponent("user.reg"), encoding: .utf8) {
            if let path = Self.registryValue("SteamPath", in: registry) {
                if Self.isDriveRoot(path) {
                    findings.append(Finding(
                        id: "steam.drive-root", severity: .problem,
                        title: "Steam saved a drive root (\(path)) as its install folder",
                        detail: "Steam then reports the install folder as not writable and asks to repair on every start.",
                        suggestion: "Start Steam through a folder (PearSilicon 0.8.6+ uses C:\\PearSiliconSteam); Steam saves the new path itself."))
                } else {
                    findings.append(Finding(id: "steam.install-path", severity: .info, title: "Saved install folder: \(path)", detail: ""))
                }
            } else {
                findings.append(Finding(id: "steam.not-started", severity: .info,
                                        title: "Steam has not saved an install folder yet",
                                        detail: "Its first start has not finished in this prefix.",
                                        suggestion: "Errors of a failed first start are in PearSilicon's Wine log for this entry: `peardebug analyze <log>`."))
            }
        }

        // Where the CEF children were started from, in the newest session.
        let webhelper = Self.newestSession(read(logs, "webhelper.txt"))
        let launches = webhelper.filter { $0.contains("launching child process with:") || $0.contains("webhelper launched") }
        if launches.contains(where: { $0.lowercased().contains("\\steam\\bin\\cef\\") }) {
            findings.append(Finding(
                id: "steam.cef-path-rule", severity: .warning,
                title: "Steam's browser processes run from a path containing \\Steam\\bin\\cef\\",
                detail: launches.prefix(2).map { String($0.prefix(200)) }.joined(separator: "\n"),
                suggestion: "MetalSharp's launcher rule matches this path and can crash StorageService. Start Steam through PearSilicon 0.8.6+."))
        }
        let fatal = (read(logs, "cef_log.txt") + "\n" + webhelper.joined(separator: "\n"))
            .split(whereSeparator: \.isNewline).filter { $0.contains("target_services.cc") || $0.contains("process_state_.InitCalled") }
        if !fatal.isEmpty {
            findings.append(Finding(id: "steam.cef-storage-fatal", severity: .critical,
                                    title: "Steam's browser (StorageService) failed to start",
                                    detail: fatal.prefix(2).map { String($0.prefix(200)) }.joined(separator: "\n"),
                                    suggestion: "See steam.cef-path-rule."))
        }

        // UI closed right after it was started: a Steam dialog is waiting.
        let html = Self.newestSession(read(logs, "steamui_html.txt"))
        for (index, line) in html.enumerated() where line.contains("Started webhelper process") {
            let pid = line.components(separatedBy: "Started webhelper process ").last ?? ""
            if html.dropFirst(index + 1).prefix(3).contains(where: { $0.contains("Shutting down webhelper process \(pid)") }) {
                findings.append(Finding(
                    id: "steam.ui-closed-at-start", severity: .problem,
                    title: "Steam closed its window right after starting it",
                    detail: line + "\n(shut down immediately)",
                    suggestion: "Steam is showing a dialog of its own, such as the install-folder repair prompt. Check steam.drive-root and look for a Steam Error window."))
                break
            }
        }

        if let shown = webhelper.last(where: { $0.contains("DesktopLoginWindow") && $0.contains("WasHidden 0") }) {
            findings.append(Finding(id: "steam.login-window", severity: .info, title: "The Steam sign-in window was shown",
                                    detail: String(shown.prefix(80))))
        }
        if let logon = Self.newestSession(read(logs, "steamui_login.txt")).last(where: { $0.contains("logon failure") || $0.contains("LogonFailure") }) {
            findings.append(Finding(id: "steam.logon-failure", severity: .warning, title: "Steam could not reach its servers once",
                                    detail: logon, suggestion: "Usually temporary; Steam retries by itself."))
        }
        return findings.sorted { ($0.severity, $1.id) > ($1.severity, $0.id) }
    }

    // MARK: - Helpers

    private func read(_ folder: URL, _ name: String) -> String {
        (try? String(contentsOf: folder.appendingPathComponent(name), encoding: .utf8))
            ?? (try? String(contentsOf: folder.appendingPathComponent(name), encoding: .isoLatin1)) ?? ""
    }

    private func lastMatch(in text: String, after marker: String) -> String? {
        text.split(whereSeparator: \.isNewline).last { $0.contains(marker) }
            .flatMap { $0.components(separatedBy: marker).last }
            .map { $0.trimmingCharacters(in: .whitespaces) }
    }

    /// Lines of the newest run: Steam appends to its logs, and each run starts
    /// with a "Client version" or "Startup" line.
    static func newestSession(_ text: String) -> [String] {
        let lines = text.split(whereSeparator: \.isNewline).map(String.init)
        guard let start = lines.lastIndex(where: { $0.contains("Client version:") || $0.contains("Startup - ") }) else { return lines }
        return Array(lines[start...])
    }

    /// `"SteamPath"="c:/pearsiliconsteam"` → `c:/pearsiliconsteam`.
    static func registryValue(_ name: String, in registry: String) -> String? {
        let marker = "\"\(name)\"=\""
        guard let range = registry.range(of: marker) else { return nil }
        let value = registry[range.upperBound...].prefix { $0 != "\"" }
        return String(value).replacingOccurrences(of: "\\\\", with: "\\")
    }

    /// `s:`, `S:\`, `s:/` are drive roots; `c:/steam` is not.
    static func isDriveRoot(_ path: String) -> Bool {
        path.range(of: #"^[A-Za-z]:[\\/]?$"#, options: .regularExpression) != nil
    }
}
