import Foundation
#if canImport(Darwin)
import Darwin
#endif

/// Minimal GDB remote serial protocol client for PearLegacyEngine's `--gdb`
/// server (armv7 guest). It supports what the engine supports: register and
/// memory access, continue, single-step and software breakpoints by trap write.
public final class GDBRemoteClient {
    public enum ProbeError: Error, CustomStringConvertible {
        case connect(String), protocolError(String)
        public var description: String {
            switch self { case .connect(let m), .protocolError(let m): m }
        }
    }

    private let socket: Int32
    private var buffer = [UInt8]()
    /// Thread that reported the last stop (GDB thread ids are 1-based).
    public private(set) var stoppedThread: String = "1"

    public init(host: String = "127.0.0.1", port: UInt16) throws {
        socket = Darwin.socket(AF_INET, SOCK_STREAM, 0)
        guard socket >= 0 else { throw ProbeError.connect("socket() failed") }
        var address = sockaddr_in()
        address.sin_family = sa_family_t(AF_INET)
        address.sin_port = port.bigEndian
        inet_pton(AF_INET, host, &address.sin_addr)
        let result = withUnsafePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                Darwin.connect(socket, $0, socklen_t(MemoryLayout<sockaddr_in>.size))
            }
        }
        guard result == 0 else { throw ProbeError.connect("cannot connect to \(host):\(port)") }
        try sendRaw("+")  // the engine expects an initial acknowledgement
    }

    deinit { close(socket) }

    private func sendRaw(_ string: String) throws {
        let bytes = Array(string.utf8)
        guard bytes.withUnsafeBytes({ Darwin.send(socket, $0.baseAddress, $0.count, 0) }) == bytes.count else {
            throw ProbeError.protocolError("send failed")
        }
    }

    private func receivePacket() throws -> String {
        while true {
            if let start = buffer.firstIndex(of: UInt8(ascii: "$")),
               let hash = buffer[start...].firstIndex(of: UInt8(ascii: "#")),
               buffer.count >= hash + 3 {
                let body = String(decoding: buffer[(start + 1)..<hash], as: UTF8.self)
                buffer.removeSubrange(0..<(hash + 3))
                try sendRaw("+")
                return body
            }
            var chunk = [UInt8](repeating: 0, count: 65_536)
            let count = chunk.withUnsafeMutableBytes { recv(socket, $0.baseAddress, $0.count, 0) }
            guard count > 0 else { throw ProbeError.protocolError("connection closed by the engine") }
            buffer.append(contentsOf: chunk[0..<count])
        }
    }

    @discardableResult
    public func command(_ body: String) throws -> String {
        let checksum = body.utf8.reduce(UInt8(0)) { $0 &+ $1 }
        try sendRaw("$\(body)#\(String(format: "%02x", checksum))")
        return try receivePacket()
    }

    /// Resumes all threads; returns when the guest stops again.
    @discardableResult
    public func continueExecution() throws -> String {
        let reply = try command("c")
        if let range = reply.range(of: "thread:") {
            stoppedThread = String(reply[range.upperBound...].prefix { $0 != ";" })
        }
        return reply
    }

    public func step() throws {
        try command("Hc\(stoppedThread)")
        try command("s")
    }

    /// r0…r15 of the stopped thread.
    public func registers() throws -> [UInt32] {
        try command("Hg\(stoppedThread)")
        let hex = try command("g")
        var registers: [UInt32] = []
        var index = hex.startIndex
        for _ in 0..<16 {
            guard let end = hex.index(index, offsetBy: 8, limitedBy: hex.endIndex) else { break }
            let word = UInt32(hex[index..<end], radix: 16) ?? 0
            registers.append(word.byteSwapped)
            index = end
        }
        return registers
    }

    public func readMemory(_ address: UInt32, count: Int) throws -> [UInt8] {
        let reply = try command(String(format: "m%x,%x", address, count))
        guard !reply.hasPrefix("E") else { throw ProbeError.protocolError("cannot read \(String(format: "%#x", address))") }
        var bytes: [UInt8] = []
        var index = reply.startIndex
        while let end = reply.index(index, offsetBy: 2, limitedBy: reply.endIndex), index < reply.endIndex {
            bytes.append(UInt8(reply[index..<end], radix: 16) ?? 0)
            index = end
        }
        return bytes
    }

    public func writeMemory(_ address: UInt32, _ bytes: [UInt8]) throws {
        let hex = bytes.map { String(format: "%02x", $0) }.joined()
        let reply = try command(String(format: "M%x,%x:", address, bytes.count) + hex)
        guard reply == "OK" else { throw ProbeError.protocolError("cannot write \(String(format: "%#x", address))") }
    }

    public func detach() { _ = try? sendRaw("$D#44") }
}

/// Watches guest functions at their entry and reports arguments and return
/// values. Arm-mode functions are trapped with UDF, Thumb returns with BKPT.
public struct LegacyProbe {
    public struct Hit: Sendable {
        public var name: String
        public var arguments: [UInt32]
        public var returnValue: UInt32
        public var caller: UInt32
    }

    static let armTrap: [UInt8] = [0xfe, 0xde, 0xff, 0xe7]
    static let thumbTrap: [UInt8] = [0x00, 0xbe]

    public let client: GDBRemoteClient
    public var functions: [UInt32: String]

    public init(client: GDBRemoteClient, functions: [UInt32: String]) {
        self.client = client
        self.functions = functions
    }

    /// Runs until `count` function calls have been observed.
    public func record(count: Int, onHit: (Hit) -> Void) throws {
        var original: [UInt32: [UInt8]] = [:]
        for address in functions.keys { original[address] = try client.readMemory(address, count: 4) }
        func arm(_ on: Bool) throws {
            for (address, bytes) in original { try client.writeMemory(address, on ? Self.armTrap : bytes) }
        }
        try arm(true)
        defer { try? arm(false) }
        var seen = 0
        while seen < count {
            try client.continueExecution()
            let registers = try client.registers()
            guard registers.count == 16, let name = functions[registers[15]] else { continue }
            seen += 1
            let link = registers[14]
            let returnAddress = link & ~1
            let thumb = link & 1 == 1
            try arm(false)
            let saved = try client.readMemory(returnAddress, count: thumb ? 2 : 4)
            try client.writeMemory(returnAddress, thumb ? Self.thumbTrap : Self.armTrap)
            try client.continueExecution()
            let after = try client.registers()
            try client.writeMemory(returnAddress, saved)
            try arm(true)
            onHit(Hit(name: name, arguments: Array(registers[0..<4]), returnValue: after.first ?? 0, caller: link))
        }
    }
}

/// Symbol lookup for a guest Mach-O via `nm` (armv7 slice).
public struct GuestSymbols {
    public private(set) var byName: [String: UInt32] = [:]
    private var sorted: [(UInt32, String)] = []

    public init(binary: URL, arch: String = "armv7") throws {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/xcrun")
        process.arguments = ["nm", "-arch", arch, "-n", binary.path]
        let pipe = Pipe()
        process.standardOutput = pipe
        process.standardError = FileHandle.nullDevice
        try process.run()
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        for line in String(decoding: data, as: UTF8.self).split(separator: "\n") {
            let parts = line.split(separator: " ")
            guard parts.count == 3, parts[1] != "U", let address = UInt32(parts[0], radix: 16) else { continue }
            let name = String(parts[2])
            byName[name] = address
            sorted.append((address, name))
        }
    }

    public func address(of name: String) -> UInt32? {
        byName[name] ?? byName["_" + name]
    }

    /// `symbol+offset` for an address.
    public func describe(_ address: UInt32) -> String {
        let target = address & ~1
        var low = 0, high = sorted.count - 1, best: (UInt32, String)?
        while low <= high {
            let mid = (low + high) / 2
            if sorted[mid].0 <= target { best = sorted[mid]; low = mid + 1 } else { high = mid - 1 }
        }
        guard let best else { return String(format: "%#x", address) }
        return "\(best.1)+\(String(format: "%#x", target - best.0))"
    }
}
