import Darwin
import ImageIO
import Foundation
import Testing
@testable import PearDebugKit

// Log lines below are taken from real runs on 2026-11-12.

private func findingIDs(_ text: String, _ source: LogSource) -> Set<String> {
    Set(Analyzer().analyze(text, source: source).findings.map(\.id))
}

@Test func legacyNullPageAndDirectoryOpen() {
    let log = """
    PearLegacyEngine::libc::posix_io: Note: open() of directory "/var/mobile/Applications/0/Asphalt6.app/data" with 0x0
    PearLegacyEngine::fs: Warning: Seeking a directory as a guest file!
    thread 'main' (512314) panicked at src/mem.rs:357:9:
    Attempted null-page access at 0x8 (0x4 bytes)
    """
    let ids = findingIDs(log, .legacyEngine)
    #expect(ids.contains("legacy.null-page"))
    #expect(ids.contains("legacy.open-directory"))
    #expect(ids.contains("legacy.panic"))
}

@Test func steamCEFFatalAndBreakpointsInWineLog() {
    let fatal = "[0000/000000.000:FATAL:target_services.cc(121)] Check failed: process_state_.InitCalled()."
    #expect(findingIDs("wine: starting\n" + fatal, .wine).contains("wine.steam-cef-storage"))
    let breakpoints = (1...3).map { "wine: Unhandled exception 0x80000003 in thread \($0) at address 0000000140054740" }
        .joined(separator: "\n") + "\nmullion: flight record of exception in ntdll.dll+0xc8218 in /x/Steam.exe.flight"
    let ids = findingIDs(breakpoints, .wine)
    #expect(ids.contains("wine.breakpoint"))
    #expect(ids.contains("wine.mullion-flight"))
    // Two breakpoints alone are below the threshold and not reported.
    #expect(!findingIDs("wine: Unhandled exception 0x80000003\nwine: Unhandled exception 0x80000003", .wine).contains("wine.breakpoint"))
}

@Test func hostUpdateAndRateLimitFindings() {
    let log = """
    [PearSilicon] Update download failed: The app's PearBuildCommit disagrees with the resolved release tag.
    [PearSilicon] api.github.com is rate-limiting requests (HTTP 403); try again later
    """
    let ids = findingIDs(log, .host)
    #expect(ids.contains("host.update-commit-mismatch"))
    #expect(ids.contains("host.github-rate-limit"))
}

// MARK: - Steam

private struct SteamFixture {
    let root: URL
    let steam: URL
    let prefix: URL
    init() throws {
        root = FileManager.default.temporaryDirectory.appendingPathComponent("steam-\(UUID().uuidString)")
        prefix = root.appendingPathComponent("Prefix")
        steam = prefix.appendingPathComponent("drive_c/Program Files (x86)/Steam")
        try FileManager.default.createDirectory(at: steam.appendingPathComponent("logs"), withIntermediateDirectories: true)
    }
    func log(_ name: String, _ text: String) throws {
        try text.write(to: steam.appendingPathComponent("logs/" + name), atomically: true, encoding: .utf8)
    }
    func registry(steamPath: String) throws {
        try "[Software\\\\Valve\\\\Steam] 1\n\"SteamExe\"=\"\(steamPath)/steam.exe\"\n\"SteamPath\"=\"\(steamPath)\"\n"
            .write(to: prefix.appendingPathComponent("user.reg"), atomically: true, encoding: .utf8)
    }
    func remove() { try? FileManager.default.removeItem(at: root) }
}

@Test func steamHealthyRunThroughFolderAlias() throws {
    let f = try SteamFixture(); defer { f.remove() }
    try f.registry(steamPath: "c:/pearsiliconsteam")
    try f.log("console_log.txt", "[2026-11-12 21:11:55] Client version: 1794424036\n")
    try f.log("webhelper.txt", """
    [2026-11-12 21:11:58] Client version: no bootstrapper found
    [2026-11-12 21:11:58] Startup - webhelper launched pid: 376 commandline: "C:\\PearSiliconSteam\\bin\\cef\\cef.win64\\steamwebhelper.exe" -nocrashdialog
    [2026-11-12 21:11:58] Browser - launching child process with: "C:\\PearSiliconSteam\\bin\\cef\\cef.win64\\steamwebhelper.exe" --type=utility --utility-sub-type=storage.mojom.StorageService
    [2026-11-12 21:12:12] SP DesktopLoginWindow_uid0-'Sign in to': WasHidden 0: (0, 0) 700x440
    """)
    try f.log("steamui_html.txt", "[2026-11-12 21:11:57] Client version: 1794424036\n[2026-11-12 21:11:58] Started webhelper process 376\n[2026-11-12 21:12:12] BrowserReady: handle:131073\n")
    let diagnostics = SteamDiagnostics(steamFolder: f.steam)
    #expect(diagnostics.prefix?.standardizedFileURL == f.prefix.standardizedFileURL)
    let ids = Set(diagnostics.findings().map(\.id))
    #expect(ids == ["steam.build", "steam.install-path", "steam.login-window"])
}

@Test func steamDriveRootRepairLoopAndCEFPathRule() throws {
    let f = try SteamFixture(); defer { f.remove() }
    try f.registry(steamPath: "s:")
    try f.log("webhelper.txt", """
    [2026-11-12 20:21:05] Client version: no bootstrapper found
    [2026-11-12 20:21:05] Browser - launching child process with: "C:\\Program Files (x86)\\Steam\\bin\\cef\\cef.win64\\steamwebhelper.exe" --type=utility
    [0000/000000.000:FATAL:target_services.cc(121)] Check failed: process_state_.InitCalled().
    """)
    try f.log("steamui_html.txt", "[2026-11-12 20:34:57] Client version: 1794424036\n[2026-11-12 20:34:57] Started webhelper process 380\n[2026-11-12 20:34:57] Shutting down webhelper process 380\n")
    let findings = SteamDiagnostics(steamFolder: f.steam).findings()
    let ids = Set(findings.map(\.id))
    #expect(ids.isSuperset(of: ["steam.drive-root", "steam.cef-path-rule", "steam.cef-storage-fatal", "steam.ui-closed-at-start"]))
    #expect(findings.first?.severity == .critical)
}

@Test func steamHelpers() {
    #expect(SteamDiagnostics.isDriveRoot("s:"))
    #expect(SteamDiagnostics.isDriveRoot("S:\\"))
    #expect(SteamDiagnostics.isDriveRoot("s:/"))
    #expect(!SteamDiagnostics.isDriveRoot("c:/pearsiliconsteam"))
    #expect(SteamDiagnostics.registryValue("SteamPath", in: "\"SteamPath\"=\"c:\\\\program files (x86)\\\\steam\"") == "c:\\program files (x86)\\steam")
    #expect(SteamDiagnostics.newestSession("a\nClient version: 1\nb\nClient version: 2\nc") == ["Client version: 2", "c"])
    // Steam writes Windows line endings; "\r\n" is one Swift Character.
    #expect(SteamDiagnostics.newestSession("a\r\nClient version: 1\r\nb\r\nClient version: 2\r\nc\r\n") == ["Client version: 2", "c"])
}

@Test func missingSteamLogsAreReported() {
    let ids = SteamDiagnostics(steamFolder: URL(fileURLWithPath: "/nonexistent/Steam")).findings().map(\.id)
    #expect(ids == ["steam.no-logs"])
}

// MARK: - Wine prefix processes

@Test func wineProcessScanFindsAndKillsOnlyTheTarget() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent("wineps-\(UUID().uuidString)")
    let other = FileManager.default.temporaryDirectory.appendingPathComponent("wineps-\(UUID().uuidString)")
    defer { try? FileManager.default.removeItem(at: root); try? FileManager.default.removeItem(at: other) }
    func spawn(in directory: URL) throws -> Process {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/sleep")
        process.arguments = ["60"]
        process.currentDirectoryURL = directory
        try process.run()
        return process
    }
    let game = try spawn(in: root.appendingPathComponent("Payload/Program"))
    let unrelated = try spawn(in: other.appendingPathComponent("Prefix/drive_c"))
    defer { if unrelated.isRunning { unrelated.terminate() }; if game.isRunning { game.terminate() } }
    let target = WineProcesses.Target(id: "game", root: root, prefix: root.appendingPathComponent("Prefix"))
    let scan = WineProcesses.scan([target, .init(id: "other", root: other, prefix: other.appendingPathComponent("Prefix"))])
    #expect(scan["game"]?.map(\.pid) == [game.processIdentifier])
    #expect(scan["game"]?.first?.name == "sleep")
    #expect(scan["other"]?.map(\.pid) == [unrelated.processIdentifier])
    #expect(WineProcesses.hasProgram(scan["game"]))
    let left = await WineProcesses.kill(target)
    #expect(left.isEmpty)
    game.waitUntilExit()
    #expect(game.terminationStatus == SIGKILL)
    #expect(unrelated.isRunning)
}

@Test func wineServicesAreNotPrograms() {
    let services = ["services.exe", "winedevice.exe", "explorer.exe", "plugplay.exe", "svchost.exe", "rpcss.exe"]
        .map { WineProcesses.Member(pid: 1, name: $0, isServer: false) } + [.init(pid: 2, name: "wineserver", isServer: true)]
    #expect(!WineProcesses.hasProgram(services))
    #expect(WineProcesses.hasProgram(services + [.init(pid: 3, name: "gta-vc.exe", isServer: false)]))
}

// MARK: - Frame dumps

@Test func frameDumpToPNGAndDrawnFraction() throws {
    let folder = FileManager.default.temporaryDirectory.appendingPathComponent("frames-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: folder) }
    // 4x2: top row white, bottom row black.
    var bytes = Data("4 2\n".utf8)
    bytes.append(Data(repeating: 255, count: 4 * 4))
    bytes.append(Data(repeating: 0, count: 4 * 4))
    let raw = folder.appendingPathComponent("f00000.raw")
    try bytes.write(to: raw)
    let frame = try LegacyFrameDump(contentsOf: raw)
    #expect(frame.width == 4 && frame.height == 2)
    #expect(frame.drawnFraction == 0.5)
    let png = folder.appendingPathComponent("f00000.png")
    try frame.writePNG(to: png, flipVertically: true)
    let source = try #require(CGImageSourceCreateWithURL(png as CFURL, nil))
    let image = try #require(CGImageSourceCreateImageAtIndex(source, 0, nil))
    #expect(image.width == 4 && image.height == 2)
    try Data("broken".utf8).write(to: raw)
    #expect(throws: (any Error).self) { try LegacyFrameDump(contentsOf: raw) }
}
