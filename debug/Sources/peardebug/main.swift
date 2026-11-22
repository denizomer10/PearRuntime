import Foundation
import PearDebugKit

let usage = """
peardebug — logs and diagnostics for PearSilicon components

USAGE
  peardebug analyze <log-file | bundle-id> [--source legacy|runtime|wine|mac|host]
                                            [--format text|markdown|json] [--run N]
  peardebug logs <bundle-id> [--follow] [--source S] [--run N]
  peardebug list <bundle-id>                list every log and crash report for an app
  peardebug crashes <process-name>          summarize recent macOS crash reports
  peardebug legacy probe <guest-binary> --port 9001 --watch sym1,sym2 [--count 40]
                                            watch guest functions in PearLegacyEngine
                                            (start the engine with --gdb=127.0.0.1:9001)
  peardebug legacy symbolize <guest-binary> <address>...
  peardebug legacy crash [--port 9001] [--binary <guest-binary>] [--json]
                                            run a 32-bit game to its crash and print registers,
                                            call chain and memory (engine started with --gdb)
  peardebug legacy frames <dump-prefix> [--out <folder>] [--flip]
                                            convert PEAR_DUMP_FRAME / PEAR_PRESENT_DUMP frames to
                                            PNG and report their size (--flip for PEAR_DUMP_FRAME)
  peardebug wine ps <entry-folder> [--prefix <WINEPREFIX>] [--json]
                                            every process of a Wine prefix (default prefix:
                                            <entry-folder>/Prefix), whichever engine started it
  peardebug steam <steam-folder> [--format text|markdown|json]
                                            explain a Windows Steam client's state from its logs

ENVIRONMENT
  PEARDEBUG_LOG_DIR   log directory (default: ~/Library/Application Support/PearSilicon/Logs)
"""

struct Arguments {
    var positionals: [String] = []
    var options: [String: String] = [:]
    var flags: Set<String> = []

    init(_ raw: ArraySlice<String>) {
        var iterator = raw.makeIterator()
        while let argument = iterator.next() {
            if argument.hasPrefix("--") {
                let name = String(argument.dropFirst(2))
                if ["follow", "help", "json", "flip"].contains(name) {
                    flags.insert(name)
                } else if let value = iterator.next() {
                    options[name] = value
                }
            } else {
                positionals.append(argument)
            }
        }
    }
}

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data("peardebug: \(message)\n".utf8))
    exit(1)
}

var logDirectory: URL {
    if let custom = ProcessInfo.processInfo.environment["PEARDEBUG_LOG_DIR"] {
        return URL(fileURLWithPath: custom)
    }
    return FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent("Library/Application Support/PearSilicon/Logs")
}

@MainActor func resolveLog(_ target: String, _ arguments: Arguments) -> (URL, LogSource?) {
    let source = arguments.options["source"].flatMap(LogSource.init(rawValue:))
    if FileManager.default.fileExists(atPath: target) { return (URL(fileURLWithPath: target), source) }
    let store = LogStore(directory: logDirectory)
    let run = Int(arguments.options["run"] ?? "0") ?? 0
    let candidates = store.logs(for: target).filter { $0.run == run && (source == nil || $0.source == source) }
    // Prefer the most recently written log for the app.
    let newest = candidates.max {
        let a = (try? $0.url.resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate) ?? .distantPast
        let b = (try? $1.url.resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate) ?? .distantPast
        return a < b
    }
    guard let newest else { fail("no log for '\(target)' in \(logDirectory.path)") }
    return (newest.url, newest.source)
}

let all = CommandLine.arguments
guard all.count > 1, all[1] != "--help", all[1] != "help" else {
    print(usage)
    exit(all.count > 1 ? 0 : 1)
}
let arguments = Arguments(all.dropFirst(2))

switch all[1] {
case "analyze":
    guard let target = arguments.positionals.first else { fail("analyze needs a log file or bundle id") }
    let (url, source) = resolveLog(target, arguments)
    guard let text = try? LogStore.read(url) else { fail("cannot read \(url.path)") }
    let analysis = Analyzer().analyze(text, source: source)
    let format = ReportFormat(rawValue: arguments.options["format"] ?? "text") ?? .text
    print(Report.render(analysis, title: url.lastPathComponent, format: format))
    exit(analysis.findings.contains { $0.severity >= Finding.Severity.problem } ? 2 : 0)

case "logs":
    guard let target = arguments.positionals.first else { fail("logs needs a bundle id") }
    let (url, _) = resolveLog(target, arguments)
    if arguments.flags.contains("follow") {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/tail")
        process.arguments = ["-n", "200", "-F", url.path]
        try? process.run()
        process.waitUntilExit()
    } else {
        print((try? LogStore.read(url)) ?? "", terminator: "")
    }

case "list":
    guard let target = arguments.positionals.first else { fail("list needs a bundle id") }
    let store = LogStore(directory: logDirectory)
    for entry in store.logs(for: target) {
        let size = (try? entry.url.resourceValues(forKeys: [.fileSizeKey]).fileSize) ?? 0
        print("\(entry.source.displayName.padding(toLength: 18, withPad: " ", startingAt: 0)) run \(entry.run)  \(size) B  \(entry.url.path)")
    }

case "crashes":
    guard let name = arguments.positionals.first else { fail("crashes needs a process name") }
    let reports = CrashReports.reports(matching: name)
    if reports.isEmpty { print("no crash reports for \(name)") }
    for report in reports { print(CrashReports.summarize(report)); print() }

case "legacy":
    guard let sub = arguments.positionals.first else { fail("legacy needs a subcommand") }
    func symbols(at path: String?) -> GuestSymbols? {
        guard let path else { return nil }
        guard let symbols = try? GuestSymbols(binary: URL(fileURLWithPath: path)) else { fail("cannot read symbols from \(path)") }
        return symbols
    }
    switch sub {
    case "symbolize":
        guard arguments.positionals.count >= 2, let symbols = symbols(at: arguments.positionals[1]) else { fail("symbolize needs a guest binary") }
        for raw in arguments.positionals.dropFirst(2) {
            let value = UInt32(raw.replacingOccurrences(of: "0x", with: ""), radix: 16) ?? 0
            print("\(raw) \(symbols.describe(value))")
        }
    case "probe":
        guard arguments.positionals.count >= 2, let symbols = symbols(at: arguments.positionals[1]) else { fail("probe needs a guest binary") }
        let port = UInt16(arguments.options["port"] ?? "9001") ?? 9001
        let count = Int(arguments.options["count"] ?? "40") ?? 40
        var functions: [UInt32: String] = [:]
        for name in (arguments.options["watch"] ?? "").split(separator: ",").map(String.init) {
            guard let address = symbols.address(of: name) else { fail("symbol not found: \(name)") }
            functions[address] = name
        }
        guard !functions.isEmpty else { fail("--watch needs at least one symbol") }
        do {
            let client = try GDBRemoteClient(port: port)
            print("connected; waiting for the engine to stop (PEAR_BREAK_AFTER or a breakpoint)…")
            try client.continueExecution()
            try LegacyProbe(client: client, functions: functions).record(count: count) { hit in
                let args = hit.arguments.map { String(format: "%#x", $0) }.joined(separator: ", ")
                print("\(hit.name)(\(args)) -> \(String(format: "%#x", hit.returnValue))  from \(symbols.describe(hit.caller))")
            }
            client.detach()
        } catch {
            fail("\(error)")
        }
    case "crash":
        let port = UInt16(arguments.options["port"] ?? "9001") ?? 9001
        let symbols = symbols(at: arguments.options["binary"])
        do {
            let client = try GDBRemoteClient(port: port)
            FileHandle.standardError.write(Data("connected; running the game until it stops…\n".utf8))
            let report = try LegacyCrashCapture.capture(client)
            if arguments.flags.contains("json") {
                let encoder = JSONEncoder(); encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
                print(String(decoding: try encoder.encode(report), as: UTF8.self))
            } else {
                print(report.text(symbols: symbols))
            }
        } catch {
            fail("\(error)")
        }
    case "frames":
        guard arguments.positionals.count >= 2 else { fail("frames needs a dump prefix (the value of PEAR_DUMP_FRAME)") }
        let prefix = URL(fileURLWithPath: arguments.positionals[1])
        let folder = prefix.deletingLastPathComponent()
        let stem = prefix.lastPathComponent
        let out = URL(fileURLWithPath: arguments.options["out"] ?? folder.path)
        try? FileManager.default.createDirectory(at: out, withIntermediateDirectories: true)
        let files = ((try? FileManager.default.contentsOfDirectory(atPath: folder.path)) ?? [])
            .filter { $0.hasPrefix(stem) && $0.hasSuffix(".raw") }.sorted()
        guard !files.isEmpty else { fail("no \(stem)*.raw files in \(folder.path)") }
        var sizes: [String: Int] = [:]
        for name in files {
            guard let frame = try? LegacyFrameDump(contentsOf: folder.appendingPathComponent(name)) else {
                print("\(name): unreadable"); continue
            }
            let png = out.appendingPathComponent((name as NSString).deletingPathExtension + ".png")
            do { try frame.writePNG(to: png, flipVertically: arguments.flags.contains("flip")) } catch { fail("cannot write \(png.path)") }
            sizes["\(frame.width)x\(frame.height)", default: 0] += 1
            print("\(name): \(frame.width)x\(frame.height), \(Int((frame.drawnFraction * 100).rounded()))% drawn -> \(png.lastPathComponent)")
        }
        print("sizes: " + sizes.sorted { $0.key < $1.key }.map { "\($0.key) ×\($0.value)" }.joined(separator: ", "))
    default:
        fail("unknown legacy subcommand '\(sub)'")
    }

case "wine":
    guard arguments.positionals.first == "ps", arguments.positionals.count >= 2 else { fail("usage: wine ps <entry-folder> [--prefix <WINEPREFIX>]") }
    let root = URL(fileURLWithPath: arguments.positionals[1])
    let prefix = arguments.options["prefix"].map { URL(fileURLWithPath: $0) } ?? root.appendingPathComponent("Prefix")
    let members = WineProcesses.scan([.init(id: "entry", root: root, prefix: prefix)])["entry"] ?? []
    if arguments.flags.contains("json") {
        let encoder = JSONEncoder(); encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        print(String(decoding: (try? encoder.encode(members)) ?? Data(), as: UTF8.self))
    } else {
        print(WineProcesses.hasProgram(members) ? "running (\(members.count) processes)" : (members.isEmpty ? "not running" : "only Wine services are running"))
        for member in members {
            print("  \(String(member.pid).padding(toLength: 7, withPad: " ", startingAt: 0)) \(member.name.isEmpty ? "?" : member.name)\(member.isWineService ? "  (Wine service)" : "")")
        }
    }

case "steam":
    guard let folder = arguments.positionals.first else { fail("steam needs the Steam client folder") }
    let findings = SteamDiagnostics(steamFolder: URL(fileURLWithPath: folder)).findings()
    let analysis = Analysis(source: .wine, lineCount: 0, levelCounts: [:], topAreas: [:], findings: findings)
    let format = ReportFormat(rawValue: arguments.options["format"] ?? "text") ?? .text
    print(Report.render(analysis, title: "Steam: \(folder)", format: format))
    exit(findings.contains { $0.severity >= Finding.Severity.problem } ? 2 : 0)

default:
    fail("unknown command '\(all[1])'. Run `peardebug help`.")
}
