// UINib: loads compiled iOS nibs (NIBArchive container) from the app bundle.
//
// Clean-room: the container layout was read from the apps' own resource files
// (see tools/nibdump.py for the format notes). Loading semantics follow the
// public UINib / "Resource Programming Guide" documentation: proxies resolve
// to the owner and external objects, outlet connections are made with
// key-value coding, then -awakeFromNib is sent to every decoded object.
import CoreGraphics
import Foundation
import ObjectiveC

// ---- archive ------------------------------------------------------------------

private enum PearNibValuePayload {
    case int(Int64)
    case double(Double)
    case bytes(Range<Int>)
    case null
    case ref(UInt32)
}

private struct PearNibValue {
    let key: UInt32
    let type: UInt8
    let payload: PearNibValuePayload
}

private struct PearNibObject {
    let cls: UInt32
    let first: UInt32
    let count: UInt32
}

private func readVarint(_ b: UnsafePointer<UInt8>, _ len: Int, _ i: inout Int) -> UInt32? {
    var v: UInt32 = 0
    var shift = 0
    while i < len && shift < 35 {
        let c = b[i]
        i += 1
        v |= UInt32(c & 0x7F) << shift
        if (c & 0x80) != 0 { return v }
        shift += 7
    }
    return nil
}

private func readLittle<T>(_ b: UnsafePointer<UInt8>, _ i: inout Int, as type: T.Type) -> T {
    var val: T = unsafeBitCast((), to: T.self) // overwritten immediately
    withUnsafeMutableBytes(of: &val) { dst in
        dst.copyBytes(from: UnsafeBufferPointer(start: b + i, count: MemoryLayout<T>.size))
    }
    i += MemoryLayout<T>.size
    return val
}

private func readLittleInt16(_ b: UnsafePointer<UInt8>, _ i: inout Int) -> Int16 {
    var x: Int16 = 0
    memcpy(&x, b + i, 2)
    i += 2
    return x
}

private func readLittleInt32(_ b: UnsafePointer<UInt8>, _ i: inout Int) -> Int32 {
    var x: Int32 = 0
    memcpy(&x, b + i, 4)
    i += 4
    return x
}

private func readLittleInt64(_ b: UnsafePointer<UInt8>, _ i: inout Int) -> Int64 {
    var x: Int64 = 0
    memcpy(&x, b + i, 8)
    i += 8
    return x
}

private func readLittleUInt32(_ b: UnsafePointer<UInt8>, _ i: inout Int) -> UInt32 {
    var x: UInt32 = 0
    memcpy(&x, b + i, 4)
    i += 4
    return x
}

private func readLittleFloat(_ b: UnsafePointer<UInt8>, _ i: inout Int) -> Float {
    var x: Float = 0
    memcpy(&x, b + i, 4)
    i += 4
    return x
}

private func readLittleDouble(_ b: UnsafePointer<UInt8>, _ i: inout Int) -> Double {
    var x: Double = 0
    memcpy(&x, b + i, 8)
    i += 8
    return x
}

private final class PearNibArchive {
    let data: Data
    let keys: [String]
    let classes: [String]
    let objects: [PearNibObject]
    let values: [PearNibValue]

    init?(data: Data) {
        guard data.count >= 50 else { return nil }
        let magic = Data("NIBArchive".utf8)
        guard data.prefix(10) == magic else { return nil }

        let parsed: (keys: [String], classes: [String], objects: [PearNibObject], values: [PearNibValue])? = data.withUnsafeBytes { raw in
            guard let b = raw.bindMemory(to: UInt8.self).baseAddress else { return nil }
            let len = raw.count
            var h = [UInt32](repeating: 0, count: 8)
            memcpy(&h, b + 18, 32)

            var keys: [String] = []
            keys.reserveCapacity(Int(h[2]))
            var i = Int(h[3])
            for _ in 0..<h[2] {
                guard let n = readVarint(b, len, &i), i + Int(n) <= len else { return nil }
                let s = String(bytes: UnsafeBufferPointer(start: b + i, count: Int(n)), encoding: .utf8) ?? ""
                keys.append(s)
                i += Int(n)
            }

            var classes: [String] = []
            classes.reserveCapacity(Int(h[6]))
            i = Int(h[7])
            for _ in 0..<h[6] {
                guard let n = readVarint(b, len, &i),
                      let extra = readVarint(b, len, &i) else { return nil }
                i += 4 * Int(extra)
                guard i + Int(n) <= len else { return nil }
                var nameLen = Int(n)
                while nameLen > 0 && b[i + nameLen - 1] == 0 { nameLen -= 1 }
                let s = String(bytes: UnsafeBufferPointer(start: b + i, count: nameLen), encoding: .utf8) ?? ""
                classes.append(s)
                i += Int(n)
            }

            var values: [PearNibValue] = []
            values.reserveCapacity(Int(h[4]))
            i = Int(h[5])
            for _ in 0..<h[4] {
                guard let key = readVarint(b, len, &i), i < len else { return nil }
                let type = b[i]
                i += 1
                let payload: PearNibValuePayload
                switch type {
                case 0:
                    guard i + 1 <= len else { return nil }
                    payload = .int(Int64(Int8(bitPattern: b[i])))
                    i += 1
                case 1:
                    guard i + 2 <= len else { return nil }
                    payload = .int(Int64(readLittleInt16(b, &i)))
                case 2:
                    guard i + 4 <= len else { return nil }
                    payload = .int(Int64(readLittleInt32(b, &i)))
                case 3:
                    guard i + 8 <= len else { return nil }
                    payload = .int(readLittleInt64(b, &i))
                case 4:
                    payload = .int(1)
                case 5:
                    payload = .int(0)
                case 6:
                    guard i + 4 <= len else { return nil }
                    payload = .double(Double(readLittleFloat(b, &i)))
                case 7:
                    guard i + 8 <= len else { return nil }
                    payload = .double(readLittleDouble(b, &i))
                case 8:
                    guard let n = readVarint(b, len, &i), i + Int(n) <= len else { return nil }
                    payload = .bytes(i..<(i + Int(n)))
                    i += Int(n)
                case 9:
                    payload = .null
                case 10:
                    guard i + 4 <= len else { return nil }
                    payload = .ref(readLittleUInt32(b, &i))
                default:
                    return nil
                }
                if i > len { return nil }
                values.append(PearNibValue(key: key, type: type, payload: payload))
            }

            var objects: [PearNibObject] = []
            objects.reserveCapacity(Int(h[0]))
            i = Int(h[1])
            for _ in 0..<h[0] {
                guard let cls = readVarint(b, len, &i),
                      let first = readVarint(b, len, &i),
                      let count = readVarint(b, len, &i) else { return nil }
                if Int(cls) >= classes.count || first + count > h[4] { return nil }
                objects.append(PearNibObject(cls: cls, first: first, count: count))
            }

            return (keys, classes, objects, values)
        }

        guard let parsed else { return nil }
        self.data = data
        self.keys = parsed.keys
        self.classes = parsed.classes
        self.objects = parsed.objects
        self.values = parsed.values
    }
}

// ---- decoder -------------------------------------------------------------------

private final class PearNibDecoder: NSCoder {
    let archive: PearNibArchive
    var decoded: [UInt32: AnyObject] = [:]
    var allObjects: [AnyObject] = []
    var current: UInt32 = 0
    weak var owner: AnyObject?
    var external: [String: AnyObject] = [:]

    init(archive: PearNibArchive, owner: AnyObject?, external: [String: AnyObject]) {
        self.archive = archive
        self.owner = owner
        self.external = external
        super.init()
    }

    private func pearValue(forKey key: String) -> PearNibValue? {
        let o = archive.objects[Int(current)]
        let start = Int(o.first)
        let end = start + Int(o.count)
        for idx in start..<end {
            let v = archive.values[idx]
            if archive.keys[Int(v.key)] == key {
                return v
            }
        }
        return nil
    }

    override var allowsKeyedCoding: Bool { true }
    override func containsValue(forKey key: String) -> Bool { pearValue(forKey: key) != nil }
    override var requiresSecureCoding: Bool { false }
    override func version(forClassName className: String) -> Int { 0 }

    private static func numberOf(_ v: PearNibValue?) -> Double {
        guard let v else { return 0 }
        switch v.payload {
        case .double(let d): return d
        case .int(let i): return Double(i)
        default: return 0
        }
    }

    private static func integerOf(_ v: PearNibValue?) -> Int64 {
        guard let v else { return 0 }
        switch v.payload {
        case .double(let d): return Int64(d)
        case .int(let i): return i
        default: return 0
        }
    }

    override func decodeBool(forKey key: String) -> Bool { Self.integerOf(pearValue(forKey: key)) != 0 }
    override func decodeCInt(forKey key: String) -> Int32 { Int32(truncatingIfNeeded: Self.integerOf(pearValue(forKey: key))) }
    override func decodeInt32(forKey key: String) -> Int32 { Int32(truncatingIfNeeded: Self.integerOf(pearValue(forKey: key))) }
    override func decodeInt64(forKey key: String) -> Int64 { Self.integerOf(pearValue(forKey: key)) }
    override func decodeInteger(forKey key: String) -> Int { Int(truncatingIfNeeded: Self.integerOf(pearValue(forKey: key))) }
    override func decodeFloat(forKey key: String) -> Float { Float(Self.numberOf(pearValue(forKey: key))) }
    override func decodeDouble(forKey key: String) -> Double { Self.numberOf(pearValue(forKey: key)) }

    // Geometry is stored as data: a one-byte tag (7 = doubles) followed by the values.
    private func pearDoubles(forKey key: String, count: Int) -> [Double]? {
        guard let v = pearValue(forKey: key),
              case .bytes(let range) = v.payload,
              range.count >= 1 + 8 * count else { return nil }
        return archive.data.withUnsafeBytes { raw in
            guard let b = raw.bindMemory(to: UInt8.self).baseAddress else { return nil }
            if b[range.lowerBound] != 7 { return nil }
            var out = [Double](repeating: 0, count: count)
            memcpy(&out, b + range.lowerBound + 1, 8 * count)
            return out
        }
    }

    @objc(decodeCGRectForKey:)
    override func decodeCGRect(forKey key: String) -> CGRect {
        guard let d = pearDoubles(forKey: key, count: 4) else { return .zero }
        return CGRect(x: d[0], y: d[1], width: d[2], height: d[3])
    }

    @objc(decodeCGPointForKey:)
    override func decodeCGPoint(forKey key: String) -> CGPoint {
        guard let d = pearDoubles(forKey: key, count: 2) else { return .zero }
        return CGPoint(x: d[0], y: d[1])
    }

    @objc(decodeCGSizeForKey:)
    override func decodeCGSize(forKey key: String) -> CGSize {
        guard let d = pearDoubles(forKey: key, count: 2) else { return .zero }
        return CGSize(width: d[0], height: d[1])
    }

    override func decodeObject(forKey key: String) -> Any? {
        guard let v = pearValue(forKey: key) else { return nil }
        switch v.payload {
        case .ref(let ref):
            return object(at: ref)
        case .bytes(let range):
            return archive.data.subdata(in: range)
        case .int(let i):
            if v.type == 4 || v.type == 5 { return NSNumber(value: i != 0) }
            return NSNumber(value: i)
        case .double(let d):
            return NSNumber(value: d)
        case .null:
            return nil
        }
    }

    private func pearArrayElements() -> [Any] {
        var arr: [Any] = []
        let o = archive.objects[Int(current)]
        let start = Int(o.first)
        let end = start + Int(o.count)
        for idx in start..<end {
            let pv = archive.values[idx]
            if case .ref(let ref) = pv.payload, archive.keys[Int(pv.key)] == "UINibEncoderEmptyKey" {
                if let elem = object(at: ref) {
                    arr.append(elem)
                }
            }
        }
        return arr
    }

    private func pearString(at index: UInt32) -> String? {
        object(at: index) as? String
    }

    private func instantiateObjCClass(_ cls: AnyClass) -> AnyObject? {
        let initCoder = NSSelectorFromString("initWithCoder:")
        if cls.instancesRespond(to: initCoder), let codingCls = cls as? NSCoding.Type {
            return codingCls.init(coder: self) as AnyObject?
        }
        if let nsCls = cls as? NSObject.Type {
            return nsCls.init()
        }
        return nil
    }

    // Materialize one archived object. Results are memoized so shared references
    // (an outlet target that is also a top-level object) stay identical.
    func object(at index: UInt32) -> AnyObject? {
        if Int(index) >= archive.objects.count { return nil }
        if let done = decoded[index] {
            return done === NSNull() ? nil : done
        }

        let saved = current
        current = index
        let clsName = archive.classes[Int(archive.objects[Int(index)].cls)]
        var obj: AnyObject?

        if clsName == "NSString" || clsName == "NSMutableString" {
            if let v = pearValue(forKey: "NS.bytes"), case .bytes(let range) = v.payload {
                obj = (String(data: archive.data.subdata(in: range), encoding: .utf8) ?? "") as NSString
            } else {
                obj = "" as NSString
            }
        } else if clsName == "NSArray" || clsName == "NSMutableArray" {
            obj = pearArrayElements() as NSArray
        } else if clsName == "NSNumber" {
            obj = (decodeObject(forKey: "NS.intval") as AnyObject?)
                ?? (decodeObject(forKey: "NS.dblval") as AnyObject?)
                ?? NSNumber(value: 0)
        } else if clsName == "UIColor" {
            let alpha = containsValue(forKey: "UIAlpha") ? decodeDouble(forKey: "UIAlpha") : 1.0
            obj = UIColor.color(
                red: decodeDouble(forKey: "UIRed"),
                green: decodeDouble(forKey: "UIGreen"),
                blue: decodeDouble(forKey: "UIBlue"),
                alpha: alpha
            )
        } else if clsName == "UIProxyObject" {
            let v = pearValue(forKey: "UIProxiedObjectIdentifier")
            let ident: String? = if let v, case .ref(let ref) = v.payload { pearString(at: ref) } else { nil }
            current = index
            if ident == "IBFilesOwner" {
                obj = owner
            } else if ident == "IBFirstResponder" {
                obj = nil
            } else if let ident {
                obj = external[ident]
            }
        } else if clsName == "UIClassSwapper" || clsName == "UICustomObject" {
            let cn = pearValue(forKey: "UIClassName")
            let on = pearValue(forKey: "UIOriginalClassName")
            let className: String? = if let cn, case .ref(let ref) = cn.payload { pearString(at: ref) } else { nil }
            let original: String? = if let on, case .ref(let ref) = on.payload { pearString(at: ref) } else { nil }
            current = index
            let c: AnyClass? = className.flatMap { NSClassFromString($0) } ?? original.flatMap { NSClassFromString($0) }
            if let c {
                if clsName == "UICustomObject" || original == "UICustomObject" || !c.instancesRespond(to: NSSelectorFromString("initWithCoder:")) {
                    obj = (c as? NSObject.Type)?.init()
                } else {
                    obj = instantiateObjCClass(c)
                }
            } else {
                PearMissingReport(PearMissingBehavior, "UINib", "class \(className ?? "(null)") not found", nil)
            }
        } else {
            if let c = NSClassFromString(clsName) {
                obj = instantiateObjCClass(c)
            } else {
                PearMissingReport(PearMissingBehavior, "UINib", "archived class \(clsName) not found", nil)
            }
        }

        current = saved
        decoded[index] = obj ?? NSNull()
        if let obj, !clsName.hasPrefix("NS"), clsName != "UIProxyObject" {
            allObjects.append(obj)
        }
        return obj
    }
}

// ---- UINib -----------------------------------------------------------------------

@objc(UINib)
open class UINib: NSObject {
    private var _data: Data?
    private var _name: String?

    private class func pearPathForNib(_ name: String, bundle: Bundle?) -> String? {
        let b = bundle ?? .main
        for candidate in ["\(name)~ipad", name] {
            guard var p = b.path(forResource: candidate, ofType: "nib") else { continue }
            var isDir: ObjCBool = false
            if FileManager.default.fileExists(atPath: p, isDirectory: &isDir), isDir.boolValue {
                var inner = (p as NSString).appendingPathComponent("runtime.nib")
                if !FileManager.default.fileExists(atPath: inner) {
                    inner = (p as NSString).appendingPathComponent("objects-13.0+.nib")
                }
                p = inner
            }
            return p
        }
        return nil
    }

    @objc(nibWithNibName:bundle:)
    public class func nib(withNibName name: String, bundle bundleOrNil: Bundle?) -> UINib? {
        guard let path = pearPathForNib(name, bundle: bundleOrNil),
              let data = try? Data(contentsOf: URL(fileURLWithPath: path)) else {
            PearHostLogMessage("nib", "nib \(name) not found")
            return nil
        }
        let nib = UINib.nib(withData: data, bundle: bundleOrNil)
        nib._name = name
        return nib
    }

    @objc(nibWithData:bundle:)
    public class func nib(withData data: Data, bundle bundleOrNil: Bundle?) -> UINib {
        let nib = UINib()
        nib._data = data
        return nib
    }

    @objc(instantiateWithOwner:options:)
    @discardableResult
    open func instantiate(withOwner ownerOrNil: Any?, options optionsOrNil: [AnyHashable: Any]?) -> [Any] {
        var unused: NSArray?
        return pear_instantiate(withOwner: ownerOrNil, options: optionsOrNil, visibleWindows: &unused)
    }

    @objc(pear_instantiateWithOwner:options:visibleWindows:)
    public func pear_instantiate(
        withOwner owner: Any?,
        options: [AnyHashable: Any]?,
        visibleWindows: AutoreleasingUnsafeMutablePointer<NSArray?>?
    ) -> [Any] {
        guard let data = _data, let archive = PearNibArchive(data: data), !archive.objects.isEmpty else {
            let caller = Thread.callStackReturnAddresses.dropFirst().first.map { UnsafeRawPointer(bitPattern: $0.uintValue) } ?? nil
            PearMissingReport(PearMissingBehavior, "UINib", "\(_name ?? "(data)"): not a NIBArchive (other nib formats are not supported yet)", caller)
            return []
        }

        let ext = (options?["UINibExternalObjects"] as? [String: AnyObject]) ?? [:]
        let dec = PearNibDecoder(archive: archive, owner: owner as AnyObject?, external: ext)

        dec.current = 0
        let top = (dec.decodeObject(forKey: "UINibTopLevelObjectsKey") as? [Any]) ?? []
        dec.current = 0
        _ = dec.decodeObject(forKey: "UINibObjectsKey")
        dec.current = 0
        let connections = (dec.decodeObject(forKey: "UINibConnectionsKey") as? [Any]) ?? []
        dec.current = 0
        let windows = (dec.decodeObject(forKey: "UINibVisibleWindowsKey") as? [Any]) ?? []

        // Connections are archived as UIRuntimeOutletConnection objects.
        for c in connections {
            if let conn = c as? UIRuntimeConnection {
                conn.pear_connect()
            }
        }
        let ownerObj = owner as AnyObject?
        let awakeSel = NSSelectorFromString("awakeFromNib")
        for o in dec.allObjects {
            if o !== ownerObj && o.responds(to: awakeSel) {
                _ = o.perform(awakeSel)
            }
        }

        PearHostLogMessage("nib", "\(_name ?? "(data)"): \(top.count) top-level objects, \(connections.count) connections, \(windows.count) visible window(s)")
        if let visibleWindows {
            visibleWindows.pointee = windows as NSArray
        }
        return top.filter { ($0 as AnyObject) !== ownerObj }
    }
}

// UIRuntimeOutletConnection / UIRuntimeEventConnection archive classes.
@objc(UIRuntimeConnection)
open class UIRuntimeConnection: NSObject, NSCoding {
    var _source: AnyObject?
    var _destination: AnyObject?
    var _label: String?

    @objc(initWithCoder:)
    public required init?(coder: NSCoder) {
        _source = coder.decodeObject(forKey: "UISource") as AnyObject?
        _destination = coder.decodeObject(forKey: "UIDestination") as AnyObject?
        _label = coder.decodeObject(forKey: "UILabel") as? String
        super.init()
    }

    public func encode(with coder: NSCoder) {}

    @objc(pear_connect)
    open func pear_connect() {}
}

@objc(UIRuntimeOutletConnection)
open class UIRuntimeOutletConnection: UIRuntimeConnection {
    @objc(pear_connect)
    open override func pear_connect() {
        guard let source = _source, let label = _label else { return }
        if !PearUIKitTrySetValueForKey(source, _destination, label) {
            PearMissingReport(PearMissingBehavior, class_getName(type(of: source)), "nib outlet \(label) could not be set", nil)
        }
    }
}

@objc(UIRuntimeOutletCollectionConnection)
open class UIRuntimeOutletCollectionConnection: UIRuntimeConnection {
    @objc(pear_connect)
    open override func pear_connect() {
        PearMissingReport(PearMissingBehavior, "UINib", "outlet collections", nil)
    }
}

@objc(UIRuntimeEventConnection)
open class UIRuntimeEventConnection: UIRuntimeConnection {
    @objc(pear_connect)
    open override func pear_connect() {
        PearMissingReport(PearMissingBehavior, "UINib", "target-action (event) connections", nil)
    }
}

// Generic keyed archives store geometry as strings in the documented
// NSStringFromCGRect format "{{x, y}, {w, h}}".
extension NSCoder {
    private static func pearNumbers(_ s: String?, count: Int) -> [Double] {
        var out = [Double](repeating: 0, count: count)
        let sc = Scanner(string: s ?? "")
        sc.charactersToBeSkipped = CharacterSet(charactersIn: "{}, ")
        for i in 0..<count {
            if let v = sc.scanDouble() {
                out[i] = v
            }
        }
        return out
    }

    @objc(encodeCGPoint:forKey:)
    public func encode(_ point: CGPoint, forKey key: String) {
        encode("{\(point.x), \(point.y)}" as NSString, forKey: key)
    }

    @objc(encodeCGSize:forKey:)
    public func encode(_ size: CGSize, forKey key: String) {
        encode("{\(size.width), \(size.height)}" as NSString, forKey: key)
    }

    @objc(encodeCGRect:forKey:)
    public func encode(_ rect: CGRect, forKey key: String) {
        encode("{{\(rect.origin.x), \(rect.origin.y)}, {\(rect.size.width), \(rect.size.height)}}" as NSString, forKey: key)
    }

    @objc(decodeCGPointForKey:)
    public func decodeCGPoint(forKey key: String) -> CGPoint {
        let d = Self.pearNumbers(decodeObject(forKey: key) as? String, count: 2)
        return CGPoint(x: d[0], y: d[1])
    }

    @objc(decodeCGSizeForKey:)
    public func decodeCGSize(forKey key: String) -> CGSize {
        let d = Self.pearNumbers(decodeObject(forKey: key) as? String, count: 2)
        return CGSize(width: d[0], height: d[1])
    }

    @objc(decodeCGRectForKey:)
    public func decodeCGRect(forKey key: String) -> CGRect {
        let d = Self.pearNumbers(decodeObject(forKey: key) as? String, count: 4)
        return CGRect(x: d[0], y: d[1], width: d[2], height: d[3])
    }
}

// iOS spelling of nib loading on NSBundle.
extension Bundle {
    @objc(loadNibNamed:owner:options:)
    public func loadNibNamed(_ name: String, owner: Any?, options: [AnyHashable: Any]?) -> [Any]? {
        UINib.nib(withNibName: name, bundle: self)?.instantiate(withOwner: owner, options: options)
    }
}
