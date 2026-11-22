import Foundation
import Testing
@testable import PearDebugKit

@Test func parsesRuntimeAreas() {
    let event = LogParser.parseLine("[PearRuntime][input] game controllers at bootstrap: 0", number: 3, source: .runtime)
    #expect(event.source == .runtime)
    #expect(event.area == "input")
    #expect(event.message == "game controllers at bootstrap: 0")
}

@Test func parsesLegacyEngineModules() {
    let event = LogParser.parseLine(
        "PearLegacyEngine::frameworks::uikit::ui_font: No replacement found for font Futura-Medium.",
        number: 1, source: .legacyEngine)
    #expect(event.area == "frameworks::uikit::ui_font")
    #expect(event.message.hasPrefix("No replacement"))
}

@Test func parsesWineChannels() {
    let event = LogParser.parseLine("0124:err:module:import_dll Library d3dx9_43.dll not found", number: 9, source: .wine)
    #expect(event.level == .error)
    #expect(event.area == "module")
}

@Test func detectsSources() {
    #expect(LogSource.detect(from: "PearLegacyEngine::mach_o: Loading") == .legacyEngine)
    #expect(LogSource.detect(from: "[PearRuntime] hello") == .runtime)
    #expect(LogSource.detect(from: "0024:fixme:ntdll:x") == .wine)
}

@Test func flagsControllerBroadcastReplay() {
    let log = """
    [PearRuntime][input] game controller connected: Xbox Wireless Controller extended=1 micro=1
    [PearRuntime][input] replay tick: 1 controller(s)
    [PearRuntime][input] replaying controller connect: Xbox Wireless Controller
    """
    let analysis = Analyzer().analyze(log)
    #expect(analysis.findings.contains { $0.id == "runtime.controller-broadcast" })
}

@Test func flagsCRIManaAndPanics() {
    let log = """
    PearLegacyEngine::mach_o: Loading armv7 slice
    E05121602M : Can't execute start function when CriMvPly handle status is not STOP/PLAYEND.
    thread 'main' (1) panicked at src/gdb.rs:375:21:
    """
    let ids = Set(Analyzer().analyze(log).findings.map(\.id))
    #expect(ids.contains("legacy.cri-mana"))
    #expect(ids.contains("legacy.panic"))
}

@Test func rotatesSessionsInsteadOfTruncating() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: directory) }
    let store = LogStore(directory: directory, keepRuns: 2)
    for run in 1...4 {
        let url = try store.beginSession(appID: "com.example", source: .runtime)
        try Data("run \(run)\n".utf8).write(to: url)
    }
    #expect(try String(contentsOf: store.url(for: "com.example", source: .runtime), encoding: .utf8) == "run 4\n")
    #expect(try String(contentsOf: store.url(for: "com.example", source: .runtime, run: 1), encoding: .utf8) == "run 3\n")
    #expect(try String(contentsOf: store.url(for: "com.example", source: .runtime, run: 2), encoding: .utf8) == "run 2\n")
    #expect(!FileManager.default.fileExists(atPath: store.url(for: "com.example", source: .runtime, run: 3).path))
}
