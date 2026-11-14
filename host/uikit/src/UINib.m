// UINib: loads compiled iOS nibs (NIBArchive container) from the app bundle.
//
// Clean-room: the container layout was read from the apps' own resource files
// (see tools/nibdump.py for the format notes). Loading semantics follow the
// public UINib / "Resource Programming Guide" documentation: proxies resolve
// to the owner and external objects, outlet connections are made with
// key-value coding, then -awakeFromNib is sent to every decoded object.
#import "PearUIKitPrivate.h"
#import <objc/runtime.h>

// ---- archive ------------------------------------------------------------------

typedef struct {
    uint32_t key;
    uint8_t type;
    union { int64_t i; double d; uint32_t ref; } v;
    const uint8_t *bytes;
    uint32_t length;
} PearNibValue;

typedef struct {
    uint32_t cls;
    uint32_t first;
    uint32_t count;
} PearNibObject;

@interface PearNibArchive : NSObject {
@public
    NSData *_data;
    NSMutableArray<NSString *> *_keys;
    NSMutableArray<NSString *> *_classes;
    PearNibObject *_objects;
    uint32_t _objectCount;
    PearNibValue *_values;
    uint32_t _valueCount;
}
+ (instancetype)archiveWithData:(NSData *)data;
@end

static BOOL readVarint(const uint8_t *b, size_t len, size_t *i, uint32_t *out) {
    uint32_t v = 0;
    for (int shift = 0; *i < len && shift < 35; shift += 7) {
        uint8_t c = b[(*i)++];
        v |= (uint32_t)(c & 0x7F) << shift;
        if (c & 0x80) {
            *out = v;
            return YES;
        }
    }
    return NO;
}

@implementation PearNibArchive
- (void)dealloc {
    free(_objects);
    free(_values);
}
+ (instancetype)archiveWithData:(NSData *)data {
    const uint8_t *b = data.bytes;
    size_t len = data.length;
    if (len < 50 || memcmp(b, "NIBArchive", 10) != 0) return nil;
    uint32_t h[8];
    memcpy(h, b + 18, sizeof h);
    PearNibArchive *a = [self new];
    a->_data = data;
    a->_keys = [NSMutableArray array];
    a->_classes = [NSMutableArray array];

    size_t i = h[3];
    for (uint32_t k = 0; k < h[2]; k++) {
        uint32_t n;
        if (!readVarint(b, len, &i, &n) || i + n > len) return nil;
        [a->_keys addObject:[[NSString alloc] initWithBytes:b + i length:n encoding:NSUTF8StringEncoding] ?: @""];
        i += n;
    }
    i = h[7];
    for (uint32_t c = 0; c < h[6]; c++) {
        uint32_t n, extra;
        if (!readVarint(b, len, &i, &n) || !readVarint(b, len, &i, &extra)) return nil;
        i += 4 * (size_t)extra;
        if (i + n > len) return nil;
        size_t nameLen = n;
        while (nameLen && b[i + nameLen - 1] == 0) nameLen--;
        [a->_classes addObject:[[NSString alloc] initWithBytes:b + i length:nameLen encoding:NSUTF8StringEncoding] ?: @""];
        i += n;
    }
    a->_valueCount = h[4];
    a->_values = calloc(h[4] ?: 1, sizeof(PearNibValue));
    i = h[5];
    for (uint32_t v = 0; v < h[4]; v++) {
        PearNibValue *pv = &a->_values[v];
        if (!readVarint(b, len, &i, &pv->key) || i >= len) return nil;
        pv->type = b[i++];
        switch (pv->type) {
        case 0: pv->v.i = (int8_t)b[i]; i += 1; break;
        case 1: { int16_t x; memcpy(&x, b + i, 2); pv->v.i = x; i += 2; break; }
        case 2: { int32_t x; memcpy(&x, b + i, 4); pv->v.i = x; i += 4; break; }
        case 3: { int64_t x; memcpy(&x, b + i, 8); pv->v.i = x; i += 8; break; }
        case 4: pv->v.i = 1; break;
        case 5: pv->v.i = 0; break;
        case 6: { float x; memcpy(&x, b + i, 4); pv->v.d = x; i += 4; break; }
        case 7: { double x; memcpy(&x, b + i, 8); pv->v.d = x; i += 8; break; }
        case 8: {
            uint32_t n;
            if (!readVarint(b, len, &i, &n) || i + n > len) return nil;
            pv->bytes = b + i;
            pv->length = n;
            i += n;
            break;
        }
        case 9: break;
        case 10: memcpy(&pv->v.ref, b + i, 4); i += 4; break;
        default: return nil;
        }
        if (i > len) return nil;
    }
    a->_objectCount = h[0];
    a->_objects = calloc(h[0] ?: 1, sizeof(PearNibObject));
    i = h[1];
    for (uint32_t o = 0; o < h[0]; o++) {
        PearNibObject *po = &a->_objects[o];
        if (!readVarint(b, len, &i, &po->cls) || !readVarint(b, len, &i, &po->first) ||
            !readVarint(b, len, &i, &po->count))
            return nil;
        if (po->cls >= a->_classes.count || po->first + po->count > h[4]) return nil;
    }
    return a;
}
@end

// ---- decoder -------------------------------------------------------------------

@interface PearNibDecoder : NSCoder {
@public
    PearNibArchive *_archive;
    NSMutableDictionary<NSNumber *, id> *_decoded;
    NSMutableArray *_allObjects;
    uint32_t _current;
    id _owner;
    NSDictionary *_external;
}
- (id)objectAtIndex:(uint32_t)index;
@end

@implementation PearNibDecoder

- (const PearNibValue *)pear_value:(NSString *)key {
    PearNibObject o = _archive->_objects[_current];
    for (uint32_t v = o.first; v < o.first + o.count; v++) {
        if ([_archive->_keys[_archive->_values[v].key] isEqualToString:key]) return &_archive->_values[v];
    }
    return NULL;
}

- (BOOL)allowsKeyedCoding { return YES; }
- (BOOL)containsValueForKey:(NSString *)key { return [self pear_value:key] != NULL; }
- (BOOL)requiresSecureCoding { return NO; }
- (NSInteger)versionForClassName:(NSString *)name { return 0; }

static double numberOf(const PearNibValue *v) {
    if (!v) return 0;
    return (v->type == 6 || v->type == 7) ? v->v.d : (double)v->v.i;
}
static int64_t integerOf(const PearNibValue *v) {
    if (!v) return 0;
    return (v->type == 6 || v->type == 7) ? (int64_t)v->v.d : v->v.i;
}

- (BOOL)decodeBoolForKey:(NSString *)key { return integerOf([self pear_value:key]) != 0; }
- (int)decodeIntForKey:(NSString *)key { return (int)integerOf([self pear_value:key]); }
- (int32_t)decodeInt32ForKey:(NSString *)key { return (int32_t)integerOf([self pear_value:key]); }
- (int64_t)decodeInt64ForKey:(NSString *)key { return integerOf([self pear_value:key]); }
- (NSInteger)decodeIntegerForKey:(NSString *)key { return (NSInteger)integerOf([self pear_value:key]); }
- (float)decodeFloatForKey:(NSString *)key { return (float)numberOf([self pear_value:key]); }
- (double)decodeDoubleForKey:(NSString *)key { return numberOf([self pear_value:key]); }

- (const uint8_t *)decodeBytesForKey:(NSString *)key returnedLength:(NSUInteger *)len {
    const PearNibValue *v = [self pear_value:key];
    if (len) *len = v && v->type == 8 ? v->length : 0;
    return v && v->type == 8 ? v->bytes : NULL;
}

// Geometry is stored as data: a one-byte tag (7 = doubles) followed by the values.
- (BOOL)pear_doubles:(NSString *)key into:(double *)out count:(int)n {
    const PearNibValue *v = [self pear_value:key];
    if (!v || v->type != 8 || v->length < 1 + 8 * (uint32_t)n || v->bytes[0] != 7) return NO;
    memcpy(out, v->bytes + 1, 8 * (size_t)n);
    return YES;
}
- (CGRect)decodeCGRectForKey:(NSString *)key {
    double d[4] = {0};
    [self pear_doubles:key into:d count:4];
    return CGRectMake(d[0], d[1], d[2], d[3]);
}
- (CGPoint)decodeCGPointForKey:(NSString *)key {
    double d[2] = {0};
    [self pear_doubles:key into:d count:2];
    return CGPointMake(d[0], d[1]);
}
- (CGSize)decodeCGSizeForKey:(NSString *)key {
    double d[2] = {0};
    [self pear_doubles:key into:d count:2];
    return CGSizeMake(d[0], d[1]);
}

- (id)decodeObjectForKey:(NSString *)key {
    const PearNibValue *v = [self pear_value:key];
    if (!v) return nil;
    switch (v->type) {
    case 10: return [self objectAtIndex:v->v.ref];
    case 8: return [NSData dataWithBytes:v->bytes length:v->length];
    case 4: case 5: return @(v->v.i != 0);
    case 6: case 7: return @(v->v.d);
    case 9: return nil;
    default: return @(v->v.i);
    }
}
- (id)decodeObjectOfClass:(Class)cls forKey:(NSString *)key { return [self decodeObjectForKey:key]; }
- (id)decodeObjectOfClasses:(NSSet *)classes forKey:(NSString *)key { return [self decodeObjectForKey:key]; }

- (NSArray *)pear_arrayElements {
    NSMutableArray *a = [NSMutableArray array];
    PearNibObject o = _archive->_objects[_current];
    for (uint32_t v = o.first; v < o.first + o.count; v++) {
        PearNibValue *pv = &_archive->_values[v];
        if (pv->type == 10 && [_archive->_keys[pv->key] isEqualToString:@"UINibEncoderEmptyKey"]) {
            id e = [self objectAtIndex:pv->v.ref];
            if (e) [a addObject:e];
        }
    }
    return a;
}

- (NSString *)pear_stringAt:(uint32_t)index {
    id s = [self objectAtIndex:index];
    return [s isKindOfClass:[NSString class]] ? s : nil;
}

// Materialize one archived object. Results are memoized so shared references
// (an outlet target that is also a top-level object) stay identical.
- (id)objectAtIndex:(uint32_t)index {
    if (index >= _archive->_objectCount) return nil;
    NSNumber *k = @(index);
    id done = _decoded[k];
    if (done) return done == (id)[NSNull null] ? nil : done;

    uint32_t saved = _current;
    _current = index;
    NSString *cls = _archive->_classes[_archive->_objects[index].cls];
    id obj = nil;

    if ([cls isEqualToString:@"NSString"] || [cls isEqualToString:@"NSMutableString"]) {
        NSUInteger len = 0;
        const uint8_t *b = [self decodeBytesForKey:@"NS.bytes" returnedLength:&len];
        obj = [[NSString alloc] initWithBytes:b length:len encoding:NSUTF8StringEncoding] ?: @"";
    } else if ([cls isEqualToString:@"NSArray"] || [cls isEqualToString:@"NSMutableArray"]) {
        obj = [self pear_arrayElements];
    } else if ([cls isEqualToString:@"NSNumber"]) {
        obj = [self decodeObjectForKey:@"NS.intval"] ?: [self decodeObjectForKey:@"NS.dblval"] ?: @0;
    } else if ([cls isEqualToString:@"UIColor"]) {
        obj = [UIColor colorWithRed:[self decodeDoubleForKey:@"UIRed"] green:[self decodeDoubleForKey:@"UIGreen"]
                               blue:[self decodeDoubleForKey:@"UIBlue"]
                              alpha:[self containsValueForKey:@"UIAlpha"] ? [self decodeDoubleForKey:@"UIAlpha"] : 1.0];
    } else if ([cls isEqualToString:@"UIProxyObject"]) {
        const PearNibValue *v = [self pear_value:@"UIProxiedObjectIdentifier"];
        NSString *ident = v && v->type == 10 ? [self pear_stringAt:v->v.ref] : nil;
        _current = index;
        if ([ident isEqualToString:@"IBFilesOwner"]) obj = _owner;
        else if ([ident isEqualToString:@"IBFirstResponder"]) obj = nil;
        else obj = ident ? _external[ident] : nil;
    } else if ([cls isEqualToString:@"UIClassSwapper"] || [cls isEqualToString:@"UICustomObject"]) {
        const PearNibValue *cn = [self pear_value:@"UIClassName"];
        const PearNibValue *on = [self pear_value:@"UIOriginalClassName"];
        NSString *className = cn && cn->type == 10 ? [self pear_stringAt:cn->v.ref] : nil;
        NSString *original = on && on->type == 10 ? [self pear_stringAt:on->v.ref] : nil;
        _current = index;
        Class c = NSClassFromString(className) ?: NSClassFromString(original);
        if (!c) {
            PearMissingReport(PearMissingBehavior, "UINib", [NSString stringWithFormat:@"class %@ not found", className].UTF8String, NULL);
        } else if ([cls isEqualToString:@"UICustomObject"] || [original isEqualToString:@"UICustomObject"] ||
                   ![c instancesRespondToSelector:@selector(initWithCoder:)]) {
            obj = [[c alloc] init];
        } else {
            obj = [[c alloc] initWithCoder:self];
        }
    } else {
        Class c = NSClassFromString(cls);
        if (!c) {
            PearMissingReport(PearMissingBehavior, "UINib", [NSString stringWithFormat:@"archived class %@ not found", cls].UTF8String, NULL);
        } else if ([c instancesRespondToSelector:@selector(initWithCoder:)]) {
            obj = [[c alloc] initWithCoder:self];
        } else {
            obj = [[c alloc] init];
        }
    }
    _current = saved;
    _decoded[k] = obj ?: [NSNull null];
    if (obj && ![cls hasPrefix:@"NS"] && ![cls isEqualToString:@"UIProxyObject"]) [_allObjects addObject:obj];
    return obj;
}
@end

// -awakeFromNib is an NSObject method (AppKit, which our UIKit re-exports,
// provides the empty default on macOS).
@interface NSObject (PearNibAwaking)
- (void)awakeFromNib;
@end

// ---- UINib -----------------------------------------------------------------------

@implementation UINib {
    NSData *_data;
    NSString *_name;
}

+ (NSString *)pear_pathForNib:(NSString *)name bundle:(NSBundle *)bundle {
    bundle = bundle ?: NSBundle.mainBundle;
    for (NSString *candidate in @[ [name stringByAppendingString:@"~ipad"], name ]) {
        NSString *p = [bundle pathForResource:candidate ofType:@"nib"];
        if (!p) continue;
        BOOL dir = NO;
        if ([NSFileManager.defaultManager fileExistsAtPath:p isDirectory:&dir] && dir) {
            // A nib bundle: runtime.nib (or the first file) holds the archive.
            NSString *inner = [p stringByAppendingPathComponent:@"runtime.nib"];
            if (![NSFileManager.defaultManager fileExistsAtPath:inner])
                inner = [p stringByAppendingPathComponent:@"objects-13.0+.nib"];
            p = inner;
        }
        return p;
    }
    return nil;
}

+ (UINib *)nibWithNibName:(NSString *)name bundle:(NSBundle *)bundle {
    NSString *path = [self pear_pathForNib:name bundle:bundle];
    NSData *d = path ? [NSData dataWithContentsOfFile:path] : nil;
    if (!d) {
        PearHostLog("nib", "nib %s not found", name.UTF8String);
        return nil;
    }
    UINib *nib = [self nibWithData:d bundle:bundle];
    nib->_name = [name copy];
    return nib;
}

+ (UINib *)nibWithData:(NSData *)data bundle:(NSBundle *)bundle {
    UINib *nib = [self new];
    nib->_data = data;
    return nib;
}

- (NSArray *)instantiateWithOwner:(id)owner options:(NSDictionary *)options {
    return [self pear_instantiateWithOwner:owner options:options visibleWindows:NULL];
}

- (NSArray *)pear_instantiateWithOwner:(id)owner options:(NSDictionary *)options visibleWindows:(NSArray **)visible {
    PearNibArchive *archive = [PearNibArchive archiveWithData:_data];
    if (!archive || archive->_objectCount == 0) {
        PearMissingReport(PearMissingBehavior, "UINib",
                          [NSString stringWithFormat:@"%@: not a NIBArchive (other nib formats are not supported yet)", _name].UTF8String,
                          __builtin_return_address(0));
        return @[];
    }
    PearNibDecoder *dec = [PearNibDecoder new];
    dec->_archive = archive;
    dec->_decoded = [NSMutableDictionary dictionary];
    dec->_allObjects = [NSMutableArray array];
    dec->_owner = owner;
    dec->_external = options[@"UINibExternalObjects"];
    dec->_current = 0;

    NSArray *top = [dec decodeObjectForKey:@"UINibTopLevelObjectsKey"];
    dec->_current = 0;
    [dec decodeObjectForKey:@"UINibObjectsKey"];
    dec->_current = 0;
    NSArray *connections = [dec decodeObjectForKey:@"UINibConnectionsKey"];
    dec->_current = 0;
    NSArray *windows = [dec decodeObjectForKey:@"UINibVisibleWindowsKey"];

    // Connections are archived as UIRuntimeOutletConnection objects; they decode
    // into PearNibConnection instances (see below).
    for (id c in connections) {
        if ([c respondsToSelector:@selector(pear_connect)]) [c performSelector:@selector(pear_connect)];
    }
    for (id o in dec->_allObjects) {
        if (o != owner && [o respondsToSelector:@selector(awakeFromNib)]) [o awakeFromNib];
    }
    PearHostLog("nib", "%s: %lu top-level objects, %lu connections, %lu visible window(s)", _name.UTF8String ?: "(data)",
                (unsigned long)top.count, (unsigned long)connections.count, (unsigned long)windows.count);
    if (visible) *visible = windows ?: @[];
    NSMutableArray *result = [NSMutableArray array];
    for (id o in top)
        if (o != owner) [result addObject:o];
    return result;
}
@end

// UIRuntimeOutletConnection / UIRuntimeEventConnection archive classes.
@interface UIRuntimeConnection : NSObject
@end
@implementation UIRuntimeConnection {
@protected
    id _source, _destination;
    NSString *_label;
}
- (instancetype)initWithCoder:(NSCoder *)c {
    if ((self = [super init])) {
        _source = [c decodeObjectForKey:@"UISource"];
        _destination = [c decodeObjectForKey:@"UIDestination"];
        _label = [c decodeObjectForKey:@"UILabel"];
    }
    return self;
}
- (void)pear_connect {}
@end

@interface UIRuntimeOutletConnection : UIRuntimeConnection
@end
@implementation UIRuntimeOutletConnection
- (void)pear_connect {
    if (!_source || !_label) return;
    @try {
        [_source setValue:_destination forKey:_label];
    } @catch (NSException *e) {
        PearMissingReport(PearMissingBehavior, class_getName([_source class]),
                          [NSString stringWithFormat:@"nib outlet %@ could not be set", _label].UTF8String, NULL);
    }
}
@end

@interface UIRuntimeOutletCollectionConnection : UIRuntimeConnection
@end
@implementation UIRuntimeOutletCollectionConnection
- (void)pear_connect {
    PearMissingReport(PearMissingBehavior, "UINib", "outlet collections", NULL);
}
@end

@interface UIRuntimeEventConnection : UIRuntimeConnection
@end
@implementation UIRuntimeEventConnection
- (void)pear_connect {
    PearMissingReport(PearMissingBehavior, "UINib", "target-action (event) connections", NULL);
}
@end

// Generic keyed archives store geometry as strings in the documented
// NSStringFromCGRect format "{{x, y}, {w, h}}".
@implementation NSCoder (UIGeometryKeyedCoding)
static void pearNumbers(NSString *s, double *out, int n) {
    NSScanner *sc = [NSScanner scannerWithString:s ?: @""];
    sc.charactersToBeSkipped = [NSCharacterSet characterSetWithCharactersInString:@"{}, "];
    for (int i = 0; i < n; i++) [sc scanDouble:&out[i]];
}
- (void)encodeCGPoint:(CGPoint)p forKey:(NSString *)key {
    [self encodeObject:[NSString stringWithFormat:@"{%g, %g}", p.x, p.y] forKey:key];
}
- (void)encodeCGSize:(CGSize)z forKey:(NSString *)key {
    [self encodeObject:[NSString stringWithFormat:@"{%g, %g}", z.width, z.height] forKey:key];
}
- (void)encodeCGRect:(CGRect)r forKey:(NSString *)key {
    [self encodeObject:[NSString stringWithFormat:@"{{%g, %g}, {%g, %g}}", r.origin.x, r.origin.y, r.size.width, r.size.height]
                forKey:key];
}
- (CGPoint)decodeCGPointForKey:(NSString *)key {
    double d[2] = {0};
    pearNumbers([self decodeObjectForKey:key], d, 2);
    return CGPointMake(d[0], d[1]);
}
- (CGSize)decodeCGSizeForKey:(NSString *)key {
    double d[2] = {0};
    pearNumbers([self decodeObjectForKey:key], d, 2);
    return CGSizeMake(d[0], d[1]);
}
- (CGRect)decodeCGRectForKey:(NSString *)key {
    double d[4] = {0};
    pearNumbers([self decodeObjectForKey:key], d, 4);
    return CGRectMake(d[0], d[1], d[2], d[3]);
}
@end

// iOS spelling of nib loading on NSBundle.
@implementation NSBundle (PearUINib)
- (NSArray *)loadNibNamed:(NSString *)name owner:(id)owner options:(NSDictionary *)options {
    return [[UINib nibWithNibName:name bundle:self] instantiateWithOwner:owner options:options];
}
@end
