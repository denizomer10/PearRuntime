//! Shared runtime helpers for the Mac Catalyst modules: logging, module
//! gating, method replacement, block ABI and libdispatch.

use pear_objc::{cls, msg0, nsstring, sel, Class, Id, Imp, Sel};
use std::ffi::{c_char, c_int, c_void, CStr, CString};

pub type Method = *mut c_void;

extern "C" {
    pub fn class_getInstanceMethod(cls: Class, name: Sel) -> Method;
    pub fn class_getClassMethod(cls: Class, name: Sel) -> Method;
    pub fn class_copyMethodList(cls: Class, count: *mut u32) -> *mut Method;
    pub fn method_getName(m: Method) -> Sel;
    pub fn method_getImplementation(m: Method) -> Imp;
    pub fn method_setImplementation(m: Method, imp: Imp) -> Imp;
    pub fn method_getNumberOfArguments(m: Method) -> u32;
    pub fn class_addMethod(cls: Class, name: Sel, imp: Imp, types: *const c_char) -> bool;
    pub fn class_getSuperclass(cls: Class) -> Class;
    pub fn class_getName(cls: Class) -> *const c_char;
    pub fn object_getClass(obj: Id) -> Class;
    pub fn sel_getName(s: Sel) -> *const c_char;
    pub fn free(p: *mut c_void);
    pub fn getenv(name: *const c_char) -> *const c_char;
    pub fn dlopen(path: *const c_char, mode: c_int) -> *mut c_void;
    pub fn strcasestr(haystack: *const c_char, needle: *const c_char) -> *const c_char;
    fn _Block_copy(block: *const c_void) -> *mut c_void;
    fn _Block_release(block: *const c_void);
    static _dispatch_main_q: c_void;
    fn dispatch_async_f(queue: *const c_void, context: *mut c_void, work: extern "C" fn(*mut c_void));

    // core: PearRuntime.mm (C++ linkage) until core is ported.
    #[link_name = "_Z7PearLogP8NSStringz"]
    fn PearLog(format: Id, ...);
}

pub const RTLD_LAZY: c_int = 0x1;
pub const RTLD_GLOBAL: c_int = 0x8;

/// `[PearRuntime]…` line through PearLog (NSLog + PEAR_LOG_FILE).
pub fn log(message: &str) {
    let c = CString::new(message.replace('\0', "")).unwrap_or_default();
    unsafe { PearLog(nsstring("%s"), c.as_ptr()) };
}

/// The environment variable is set (any value, even empty), as `getenv() != NULL`.
pub fn env_set(name: &str) -> bool {
    let c = CString::new(name).unwrap_or_default();
    unsafe { !getenv(c.as_ptr()).is_null() }
}

/// `PEAR_MODULE_GUARD()`: false when the module is switched off for this launch.
pub fn module_enabled(module: &str) -> bool {
    crate::core::registry::enabled(module)
}

pub unsafe fn cstr<'a>(p: *const c_char) -> &'a str {
    if p.is_null() {
        ""
    } else {
        CStr::from_ptr(p).to_str().unwrap_or("")
    }
}

pub unsafe fn class_name(c: Class) -> &'static str {
    cstr(class_getName(c))
}

pub unsafe fn dlopen_path(path: &str, mode: c_int) -> *mut c_void {
    let c = CString::new(path).unwrap_or_default();
    dlopen(c.as_ptr(), mode)
}

/// `NSClassFromString`.
pub unsafe fn class_named(name: &str) -> Class {
    cls(name)
}

/// Replaces an existing method's implementation, or adds it with `types`.
/// Returns the previous implementation (null when the method was added).
pub unsafe fn replace_instance(c: Class, selector: &str, imp: Imp, types: &str) -> Imp {
    if c.is_null() {
        return std::ptr::null();
    }
    let s = sel(selector);
    let m = class_getInstanceMethod(c, s);
    if !m.is_null() {
        method_setImplementation(m, imp)
    } else {
        let t = CString::new(types).unwrap_or_default();
        class_addMethod(c, s, imp, t.as_ptr());
        std::ptr::null()
    }
}

/// Class-method counterpart of [`replace_instance`].
pub unsafe fn replace_class(c: Class, selector: &str, imp: Imp, types: &str) -> Imp {
    if c.is_null() {
        return std::ptr::null();
    }
    let s = sel(selector);
    let m = class_getClassMethod(c, s);
    if !m.is_null() {
        method_setImplementation(m, imp)
    } else {
        let t = CString::new(types).unwrap_or_default();
        class_addMethod(object_getClass(c as Id), s, imp, t.as_ptr());
        std::ptr::null()
    }
}

#[repr(C)]
struct BlockLayout {
    isa: *const c_void,
    flags: i32,
    reserved: i32,
    invoke: *const c_void,
}

/// The block's invoke function, cast to the caller's signature (first
/// argument is always the block itself).
pub unsafe fn block_invoke<F: Copy>(block: Id) -> Option<F> {
    if block.is_null() {
        return None;
    }
    let invoke = (*(block as *const BlockLayout)).invoke;
    if invoke.is_null() {
        None
    } else {
        Some(std::mem::transmute_copy(&invoke))
    }
}

struct MainCall {
    block: *mut c_void,
    call: unsafe fn(Id, usize),
    arg: usize,
}

extern "C" fn main_call_trampoline(ctx: *mut c_void) {
    unsafe {
        let job = Box::from_raw(ctx as *mut MainCall);
        (job.call)(job.block as Id, job.arg);
        _Block_release(job.block);
    }
}

/// `dispatch_async(main, ^{ call(block, arg); })`, keeping the block alive.
pub unsafe fn async_main_with_block(block: Id, arg: usize, call: unsafe fn(Id, usize)) {
    if block.is_null() {
        return;
    }
    let job = Box::new(MainCall { block: _Block_copy(block), call, arg });
    dispatch_async_f(&_dispatch_main_q, Box::into_raw(job) as *mut c_void, main_call_trampoline);
}

/// Entry of the `__DATA,__interpose` table dyld applies at load.
#[repr(C)]
pub struct Interpose {
    pub replacement: *const c_void,
    pub replacee: *const c_void,
}
unsafe impl Sync for Interpose {}

/// `msg0` returning an autoreleased object from a +1 one.
pub unsafe fn autoreleased(obj: Id) -> Id {
    pear_objc::autoreleased(obj)
}

pub unsafe fn ns_string(obj: Id) -> String {
    pear_objc::nsstring_to_string(obj).unwrap_or_default()
}

pub unsafe fn description(obj: Id) -> String {
    if obj.is_null() {
        return String::new();
    }
    ns_string(msg0(obj, "description"))
}

// ---------------------------------------------------------------------------
// Blocks from Rust closures (Block ABI: stack block with copy/dispose helpers,
// copied to the heap by `_Block_copy`; the closure is shared via Arc).

extern "C" {
    static _NSConcreteStackBlock: c_void;
}

const BLOCK_HAS_COPY_DISPOSE: i32 = 1 << 25;

#[repr(C)]
struct BlockDescriptor {
    reserved: usize,
    size: usize,
    copy: unsafe extern "C" fn(*mut c_void, *const c_void),
    dispose: unsafe extern "C" fn(*const c_void),
}

#[repr(C)]
struct ClosureBlock<F> {
    isa: *const c_void,
    flags: i32,
    reserved: i32,
    invoke: *const c_void,
    descriptor: *const BlockDescriptor,
    closure: *const F,
}

unsafe extern "C" fn block_copy_helper<F>(dst: *mut c_void, src: *const c_void) {
    let src = &*(src as *const ClosureBlock<F>);
    std::sync::Arc::increment_strong_count(src.closure);
    (*(dst as *mut ClosureBlock<F>)).closure = src.closure;
}

unsafe extern "C" fn block_dispose_helper<F>(block: *const c_void) {
    let b = &*(block as *const ClosureBlock<F>);
    std::sync::Arc::decrement_strong_count(b.closure);
}

/// Heap block (+1, release with `release_block`) around `f`; `invoke` is the
/// block's C entry point, which receives the block as its first argument.
unsafe fn make_block<F>(f: F, invoke: *const c_void) -> Id {
    let closure = std::sync::Arc::into_raw(std::sync::Arc::new(f));
    let descriptor = Box::leak(Box::new(BlockDescriptor {
        reserved: 0,
        size: std::mem::size_of::<ClosureBlock<F>>(),
        copy: block_copy_helper::<F>,
        dispose: block_dispose_helper::<F>,
    }));
    let stack = ClosureBlock {
        isa: &_NSConcreteStackBlock,
        flags: BLOCK_HAS_COPY_DISPOSE,
        reserved: 0,
        invoke,
        descriptor,
        closure,
    };
    let heap = _Block_copy(&stack as *const _ as *const c_void) as Id;
    std::sync::Arc::decrement_strong_count(closure); // the heap copy holds its own
    heap
}

pub unsafe fn release_block(block: Id) {
    if !block.is_null() {
        _Block_release(block);
    }
}

unsafe extern "C" fn invoke0<F: Fn()>(block: *const ClosureBlock<F>) {
    (*(*block).closure)()
}
unsafe extern "C" fn invoke1<F: Fn(Id)>(block: *const ClosureBlock<F>, a: Id) {
    (*(*block).closure)(a)
}

/// `^{ … }` as a heap block (+1).
pub unsafe fn block0<F: Fn() + 'static>(f: F) -> Id {
    make_block(f, invoke0::<F> as *const c_void)
}

/// `^(id arg){ … }` as a heap block (+1).
pub unsafe fn block1<F: Fn(Id) + 'static>(f: F) -> Id {
    make_block(f, invoke1::<F> as *const c_void)
}

// ---------------------------------------------------------------------------
// libdispatch with closures

extern "C" {
    fn dispatch_time(when: u64, delta: i64) -> u64;
    fn dispatch_after_f(when: u64, queue: *const c_void, context: *mut c_void, work: extern "C" fn(*mut c_void));
}

extern "C" fn run_boxed(ctx: *mut c_void) {
    unsafe {
        let f = Box::from_raw(ctx as *mut Box<dyn FnOnce()>);
        f();
    }
}

/// `dispatch_after(now + seconds, main queue, f)`.
pub fn after_main<F: FnOnce() + 'static>(seconds: f64, f: F) {
    let boxed: Box<Box<dyn FnOnce()>> = Box::new(Box::new(f));
    unsafe {
        let when = dispatch_time(0, (seconds * 1e9) as i64);
        dispatch_after_f(when, &_dispatch_main_q, Box::into_raw(boxed) as *mut c_void, run_boxed);
    }
}

/// `dispatch_async(main queue, f)`.
pub fn async_main<F: FnOnce() + 'static>(f: F) {
    let boxed: Box<Box<dyn FnOnce()>> = Box::new(Box::new(f));
    unsafe { dispatch_async_f(&_dispatch_main_q, Box::into_raw(boxed) as *mut c_void, run_boxed) };
}

/// `[NSNotificationCenter.defaultCenter addObserverForName:… queue:mainQueue usingBlock:]`.
pub unsafe fn observe_main(name: &str, handler: impl Fn(Id) + 'static) -> Id {
    let center: Id = msg0(cls("NSNotificationCenter"), "defaultCenter");
    let queue: Id = msg0(cls("NSOperationQueue"), "mainQueue");
    let block = block1(handler);
    let token: Id = pear_objc::msg4(center, "addObserverForName:object:queue:usingBlock:", nsstring(name), std::ptr::null_mut::<c_void>(), queue, block);
    release_block(block);
    pear_objc::objc_retain(token)
}

// ---------------------------------------------------------------------------
// Classes defined at run time

extern "C" {
    fn objc_allocateClassPair(superclass: Class, name: *const c_char, extra: usize) -> Class;
    fn objc_registerClassPair(cls: Class);
    fn class_addIvar(cls: Class, name: *const c_char, size: usize, alignment: u8, types: *const c_char) -> bool;
    fn class_getInstanceVariable(cls: Class, name: *const c_char) -> *mut c_void;
    fn ivar_getOffset(ivar: *mut c_void) -> isize;
}

/// A class whose instances carry a `T` (zero-initialized) in an ivar named
/// `pear_state`. Methods are added by the caller before `register`.
pub struct ClassBuilder {
    cls: Class,
}

impl ClassBuilder {
    pub unsafe fn new<T>(name: &str, superclass: Class) -> Option<ClassBuilder> {
        let n = CString::new(name).ok()?;
        let c = objc_allocateClassPair(superclass, n.as_ptr(), 0);
        if c.is_null() {
            return None;
        }
        if std::mem::size_of::<T>() > 0 {
            let align = std::mem::align_of::<T>().trailing_zeros() as u8;
            class_addIvar(c, c"pear_state".as_ptr(), std::mem::size_of::<T>(), align, c"?".as_ptr());
        }
        Some(ClassBuilder { cls: c })
    }

    pub unsafe fn method(&self, selector: &str, imp: Imp, types: &str) -> &Self {
        let t = CString::new(types).unwrap_or_default();
        class_addMethod(self.cls, sel(selector), imp, t.as_ptr());
        self
    }

    pub unsafe fn register(self) -> Class {
        objc_registerClassPair(self.cls);
        self.cls
    }
}

/// The `T` stored in `obj`'s `pear_state` ivar.
pub unsafe fn ivar_state<'a, T>(obj: Id) -> &'a mut T {
    let ivar = class_getInstanceVariable(object_getClass(obj), c"pear_state".as_ptr());
    &mut *((obj as *mut u8).offset(ivar_getOffset(ivar)) as *mut T)
}

/// `PearPrimaryWindow()` from the input module (C++ linkage) until ported.
pub unsafe fn primary_window() -> Id {
    extern "C" {
        #[link_name = "_Z17PearPrimaryWindowv"]
        fn PearPrimaryWindow() -> Id;
    }
    PearPrimaryWindow()
}

extern "C" {
    static _dispatch_source_type_timer: c_void;
    fn dispatch_source_create(kind: *const c_void, handle: usize, mask: usize, queue: *const c_void) -> *mut c_void;
    fn dispatch_source_set_timer(source: *mut c_void, start: u64, interval: u64, leeway: u64);
    fn dispatch_source_set_event_handler_f(source: *mut c_void, handler: extern "C" fn(*mut c_void));
    fn dispatch_set_context(object: *mut c_void, context: *mut c_void);
    fn dispatch_resume(object: *mut c_void);
}

/// Repeating main-queue timer running `handler` (never cancelled).
pub fn main_timer(start_after: f64, interval: f64, leeway: f64, handler: extern "C" fn(*mut c_void)) {
    unsafe {
        let t = dispatch_source_create(&_dispatch_source_type_timer, 0, 0, &_dispatch_main_q);
        if t.is_null() {
            return;
        }
        dispatch_source_set_timer(t, dispatch_time(0, (start_after * 1e9) as i64), (interval * 1e9) as u64, (leeway * 1e9) as u64);
        dispatch_set_context(t, std::ptr::null_mut());
        dispatch_source_set_event_handler_f(t, handler);
        dispatch_resume(t);
    }
}

/// `CGRectEqualToRect`.
pub fn rect_eq(a: pear_objc::CGRect, b: pear_objc::CGRect) -> bool {
    a == b
}

/// `[CATransaction begin]; setDisableActions:YES; f(); [CATransaction commit]`.
pub unsafe fn without_actions(f: impl FnOnce()) {
    let ca = cls("CATransaction");
    msg0::<()>(ca, "begin");
    pear_objc::msg1::<bool, ()>(ca, "setDisableActions:", true);
    f();
    msg0::<()>(ca, "commit");
}
