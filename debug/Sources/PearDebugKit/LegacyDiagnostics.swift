import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

/// State of a 32-bit game at the moment it crashed, read over PearLegacyEngine's
/// GDB server. Start the engine with `--gdb=127.0.0.1:<port>`; under a debugger
/// a guest memory error stops the game instead of ending the engine, so its
/// registers, call chain and memory can still be read.
public struct LegacyCrashReport: Codable, Sendable {
    public struct Frame: Codable, Sendable {
        public let framePointer: UInt32
        public let returnAddress: UInt32
    }
    public struct Words: Codable, Sendable {
        public let label: String
        public let address: UInt32
        /// nil where the memory is not readable.
        public let words: [UInt32?]
    }

    /// The stop reply, for example `T0bthread:1;` (signal 0x0b = SIGSEGV).
    public let stopReason: String
    public let thread: String
    public let registers: [UInt32]
    public let frames: [Frame]
    /// Memory at the stack pointer and at each register that points to
    /// readable memory: where the faulting pointer came from is usually here.
    public let memory: [Words]

    public var pc: UInt32 { registers.count > 15 ? registers[15] : 0 }

    public func text(symbols: GuestSymbols? = nil) -> String {
        func hex(_ value: UInt32) -> String { String(format: "0x%08x", value) }
        // The nearest exported symbol is only meaningful close to it: games
        // strip most functions, so a large offset names an unrelated one.
        func describe(_ value: UInt32) -> String {
            guard let symbols else { return hex(value) }
            let name = symbols.describe(value)
            if let plus = name.range(of: "+0x", options: .backwards),
               let offset = UInt32(name[plus.upperBound...], radix: 16), offset > 0x1000 {
                return "\(hex(value)) (no symbol)"
            }
            return "\(hex(value)) \(name)"
        }
        var lines = ["Stopped: \(stopReason) (thread \(thread))", "PC \(describe(pc))", "Registers:"]
        let names = (0..<13).map { "r\($0)" } + ["sp", "lr", "pc"]
        for (index, value) in registers.enumerated() where index < names.count {
            lines.append("  \(names[index].padding(toLength: 4, withPad: " ", startingAt: 0)) \(hex(value))")
        }
        lines.append("Call chain (frame pointer r7):")
        for frame in frames { lines.append("  return to \(describe(frame.returnAddress))   frame \(hex(frame.framePointer))") }
        for block in memory {
            let words = block.words.map { $0.map(hex) ?? "??????????" }.joined(separator: " ")
            lines.append("[\(block.label)] \(hex(block.address)): \(words)")
        }
        return lines.joined(separator: "\n")
    }
}

public enum LegacyCrashCapture {
    /// Resumes the game and waits for the next stop (the crash), then reads its
    /// state. The game is left stopped; detaching resumes it.
    public static func capture(_ client: GDBRemoteClient, frameLimit: Int = 24) throws -> LegacyCrashReport {
        let stop = try client.continueExecution()
        let registers = try client.registers()
        func word(_ address: UInt32) -> UInt32? {
            guard let bytes = try? client.readMemory(address, count: 4), bytes.count == 4 else { return nil }
            return UInt32(bytes[0]) | UInt32(bytes[1]) << 8 | UInt32(bytes[2]) << 16 | UInt32(bytes[3]) << 24
        }
        var frames: [LegacyCrashReport.Frame] = []
        var fp = registers.count > 7 ? registers[7] : 0
        var seen = Set<UInt32>()
        while fp != 0, frames.count < frameLimit, seen.insert(fp).inserted,
              let next = word(fp), let lr = word(fp &+ 4) {
            frames.append(.init(framePointer: fp, returnAddress: lr))
            fp = next
        }
        var memory: [LegacyCrashReport.Words] = []
        func dump(_ label: String, _ address: UInt32, count: Int = 8) {
            let aligned = address & ~3
            let words = (0..<count).map { word(aligned &+ UInt32($0 * 4)) }
            if words.contains(where: { $0 != nil }) { memory.append(.init(label: label, address: aligned, words: words)) }
        }
        if registers.count > 13 { dump("sp", registers[13], count: 16) }
        for (index, value) in registers.prefix(13).enumerated() where value >= 0x1000 {
            dump("r\(index)", value)
        }
        return LegacyCrashReport(stopReason: stop, thread: client.stoppedThread, registers: registers,
                                 frames: frames, memory: memory)
    }
}

/// Frames written by PearLegacyEngine's `PEAR_DUMP_FRAME=<prefix>` (the game's
/// renderbuffer, bottom-up) or `PEAR_PRESENT_DUMP_EVERY=n` +
/// `PEAR_PRESENT_DUMP_PREFIX=<prefix>` (what the window shows, top-down):
/// `<width> <height>\n` followed by RGBA bytes.
///
/// The renderbuffer size alone answers many layout questions: Miku Flick
/// rendered 640x920 (320x460 at 2x) instead of 640x960 because its view kept
/// the nib's status-bar layout.
public struct LegacyFrameDump: Sendable {
    public let width: Int
    public let height: Int
    public let pixels: Data

    public init(contentsOf url: URL) throws {
        let data = try Data(contentsOf: url)
        guard let newline = data.firstIndex(of: 0x0A),
              let header = String(data: data[data.startIndex..<newline], encoding: .ascii) else {
            throw CocoaError(.fileReadCorruptFile)
        }
        let parts = header.split(separator: " ").compactMap { Int($0) }
        guard parts.count == 2, parts[0] > 0, parts[1] > 0 else { throw CocoaError(.fileReadCorruptFile) }
        let pixels = data[(newline + 1)...]
        guard pixels.count >= parts[0] * parts[1] * 4 else { throw CocoaError(.fileReadCorruptFile) }
        width = parts[0]
        height = parts[1]
        self.pixels = Data(pixels.prefix(parts[0] * parts[1] * 4))
    }

    /// Fraction of pixels that are not black, a quick "is anything drawn" check.
    public var drawnFraction: Double {
        var drawn = 0
        pixels.withUnsafeBytes { raw in
            let bytes = raw.bindMemory(to: UInt8.self)
            for i in stride(from: 0, to: bytes.count, by: 4) where bytes[i] > 8 || bytes[i + 1] > 8 || bytes[i + 2] > 8 {
                drawn += 1
            }
        }
        return Double(drawn) / Double(width * height)
    }

    /// Writes a PNG; `flipVertically` for bottom-up renderbuffer dumps.
    public func writePNG(to url: URL, flipVertically: Bool) throws {
        var rows = pixels
        if flipVertically {
            let stride = width * 4
            rows = Data(capacity: pixels.count)
            for row in (0..<height).reversed() {
                rows.append(pixels[(pixels.startIndex + row * stride)..<(pixels.startIndex + (row + 1) * stride)])
            }
        }
        guard let provider = CGDataProvider(data: rows as CFData),
              let image = CGImage(width: width, height: height, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: width * 4,
                                  space: CGColorSpaceCreateDeviceRGB(),
                                  bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.noneSkipLast.rawValue),
                                  provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent),
              let destination = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil) else {
            throw CocoaError(.fileWriteUnknown)
        }
        CGImageDestinationAddImage(destination, image, nil)
        guard CGImageDestinationFinalize(destination) else { throw CocoaError(.fileWriteUnknown) }
    }
}
