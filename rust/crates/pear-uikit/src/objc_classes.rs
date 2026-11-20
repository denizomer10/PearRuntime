//! Objective-C classes of PearHost UIKit as static metadata (pear_objc::abi),
//! converted from host/uikit/src/UIKitClasses.m with the layout clang gave it:
//! ivars, properties and synthesized accessors of the PearUIKit.h interfaces,
//! and `.cxx_destruct` for object ivars. Behaviour lives elsewhere in this
//! crate: `pear_uikit_register_classes` (and the missing-call reporter)
//! replace these accessors and add every other method at load. iOS binaries
//! bind to the exported OBJC_CLASS_$_/OBJC_METACLASS_$_ symbols.
//!
//! Keep the layout of an existing class unchanged: apps compiled against
//! UIKit subclass these classes and the runtime slides their ivars past it.
#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types, dead_code)]

use pear_objc::abi::*;
use pear_objc::{CGPoint, CGRect, CGSize, Id, NSRange, Sel, UIEdgeInsets};
use std::ffi::c_void;

pear_objc::image_info!();

extern "C" {
    fn objc_storeStrong(location: *mut Id, value: Id);
    fn objc_storeWeak(location: *mut Id, value: Id) -> Id;
    fn objc_loadWeakRetained(location: *mut Id) -> Id;
    fn objc_destroyWeak(location: *mut Id);
    fn objc_autoreleaseReturnValue(value: Id) -> Id;
    fn objc_setProperty_nonatomic_copy(this: Id, cmd: Sel, value: Id, offset: isize);
}

/// An ivar's offset variable (slid by the runtime).
pub trait IvarOffset {
    fn offset() -> *const i32;
}

unsafe fn slot<T>(this: Id, offset: *const i32) -> *mut T {
    (this as *mut u8).offset(std::ptr::read_volatile(offset) as isize) as *mut T
}

// The accessors clang synthesizes for nonatomic properties under ARC.
unsafe extern "C" fn get_value<T: Copy, I: IvarOffset>(this: Id, _: Sel) -> T {
    *slot::<T>(this, I::offset())
}
unsafe extern "C" fn set_value<T: Copy, I: IvarOffset>(this: Id, _: Sel, value: T) {
    *slot::<T>(this, I::offset()) = value;
}
unsafe extern "C" fn set_strong<I: IvarOffset>(this: Id, _: Sel, value: Id) {
    objc_storeStrong(slot::<Id>(this, I::offset()), value);
}
unsafe extern "C" fn set_copy<I: IvarOffset>(this: Id, cmd: Sel, value: Id) {
    objc_setProperty_nonatomic_copy(this, cmd, value, std::ptr::read_volatile(I::offset()) as isize);
}
unsafe extern "C" fn get_weak<I: IvarOffset>(this: Id, _: Sel) -> Id {
    objc_autoreleaseReturnValue(objc_loadWeakRetained(slot::<Id>(this, I::offset())))
}
unsafe extern "C" fn set_weak<I: IvarOffset>(this: Id, _: Sel, value: Id) {
    objc_storeWeak(slot::<Id>(this, I::offset()), value);
}

/// `.cxx_destruct`: releases strong/copy object ivars, destroys weak ones.
unsafe fn destruct(this: Id, strong: &[*const i32], weak: &[*const i32]) {
    for &offset in strong {
        objc_storeStrong(slot::<Id>(this, offset), std::ptr::null_mut());
    }
    for &offset in weak {
        objc_destroyWeak(slot::<Id>(this, offset));
    }
}

extern "C" {
    #[link_name = "OBJC_CLASS_$_NSObject"]
    static NSObject_CLASS: c_void;
    #[link_name = "OBJC_METACLASS_$_NSObject"]
    static NSObject_META: c_void;
}

// ---- UIColor : NSObject ---------------------------------------------
pub mod UIColor {
    use super::*;

    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_CGColor: i32 = 8;
    pub struct I_CGColor;
    impl IvarOffset for I_CGColor {
        fn offset() -> *const i32 {
            &raw const OFFSET_CGColor
        }
    }

    #[link_section = "__DATA,__objc_const"]
    static METHODS: Writable<MethodList<1>> = Writable::new(MethodList {
        entsize_flags: 24,
        count: 1,
        methods: [
            MethodEntry { name: c"CGColor".as_ptr(), types: c"^{CGColor=}16@0:8".as_ptr(), imp: (get_value::<*const c_void, I_CGColor> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static IVARS: Writable<IvarList<1>> = Writable::new(IvarList {
        entsize: 32,
        count: 1,
        ivars: [
            IvarEntry { offset: &raw mut OFFSET_CGColor, name: c"_CGColor".as_ptr(), kind: c"^{CGColor=}".as_ptr(), alignment_raw: 3, size: 8 },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static PROPERTIES: Writable<PropertyList<1>> = Writable::new(PropertyList {
        entsize: 16,
        count: 1,
        properties: [
            PropertyEntry { name: c"CGColor".as_ptr(), attributes: c"T^{CGColor=},R,N,V_CGColor".as_ptr() },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 8,
        instance_size: 16,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIColor".as_ptr(),
        methods: METHODS.ptr(),
        protocols: std::ptr::null(),
        ivars: IVARS.ptr(),
        weak_ivar_layout: std::ptr::null(),
        properties: PROPERTIES.ptr(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIColor".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIColor"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: &raw const NSObject_META,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIColor"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: &raw const NSObject_CLASS,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UITraitCollection : NSObject -----------------------------------
pub mod UITraitCollection {
    use super::*;

    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_userInterfaceIdiom: i32 = 8;
    pub struct I_userInterfaceIdiom;
    impl IvarOffset for I_userInterfaceIdiom {
        fn offset() -> *const i32 {
            &raw const OFFSET_userInterfaceIdiom
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_displayScale: i32 = 16;
    pub struct I_displayScale;
    impl IvarOffset for I_displayScale {
        fn offset() -> *const i32 {
            &raw const OFFSET_displayScale
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_userInterfaceStyle: i32 = 24;
    pub struct I_userInterfaceStyle;
    impl IvarOffset for I_userInterfaceStyle {
        fn offset() -> *const i32 {
            &raw const OFFSET_userInterfaceStyle
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_horizontalSizeClass: i32 = 32;
    pub struct I_horizontalSizeClass;
    impl IvarOffset for I_horizontalSizeClass {
        fn offset() -> *const i32 {
            &raw const OFFSET_horizontalSizeClass
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_verticalSizeClass: i32 = 40;
    pub struct I_verticalSizeClass;
    impl IvarOffset for I_verticalSizeClass {
        fn offset() -> *const i32 {
            &raw const OFFSET_verticalSizeClass
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_forceTouchCapability: i32 = 48;
    pub struct I_forceTouchCapability;
    impl IvarOffset for I_forceTouchCapability {
        fn offset() -> *const i32 {
            &raw const OFFSET_forceTouchCapability
        }
    }

    #[link_section = "__DATA,__objc_const"]
    static METHODS: Writable<MethodList<6>> = Writable::new(MethodList {
        entsize_flags: 24,
        count: 6,
        methods: [
            MethodEntry { name: c"userInterfaceIdiom".as_ptr(), types: c"q16@0:8".as_ptr(), imp: (get_value::<isize, I_userInterfaceIdiom> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"displayScale".as_ptr(), types: c"d16@0:8".as_ptr(), imp: (get_value::<f64, I_displayScale> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"userInterfaceStyle".as_ptr(), types: c"q16@0:8".as_ptr(), imp: (get_value::<isize, I_userInterfaceStyle> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"horizontalSizeClass".as_ptr(), types: c"q16@0:8".as_ptr(), imp: (get_value::<isize, I_horizontalSizeClass> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"verticalSizeClass".as_ptr(), types: c"q16@0:8".as_ptr(), imp: (get_value::<isize, I_verticalSizeClass> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"forceTouchCapability".as_ptr(), types: c"q16@0:8".as_ptr(), imp: (get_value::<isize, I_forceTouchCapability> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static IVARS: Writable<IvarList<6>> = Writable::new(IvarList {
        entsize: 32,
        count: 6,
        ivars: [
            IvarEntry { offset: &raw mut OFFSET_userInterfaceIdiom, name: c"_userInterfaceIdiom".as_ptr(), kind: c"q".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_displayScale, name: c"_displayScale".as_ptr(), kind: c"d".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_userInterfaceStyle, name: c"_userInterfaceStyle".as_ptr(), kind: c"q".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_horizontalSizeClass, name: c"_horizontalSizeClass".as_ptr(), kind: c"q".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_verticalSizeClass, name: c"_verticalSizeClass".as_ptr(), kind: c"q".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_forceTouchCapability, name: c"_forceTouchCapability".as_ptr(), kind: c"q".as_ptr(), alignment_raw: 3, size: 8 },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static PROPERTIES: Writable<PropertyList<6>> = Writable::new(PropertyList {
        entsize: 16,
        count: 6,
        properties: [
            PropertyEntry { name: c"displayScale".as_ptr(), attributes: c"Td,R,N,V_displayScale".as_ptr() },
            PropertyEntry { name: c"forceTouchCapability".as_ptr(), attributes: c"Tq,R,N,V_forceTouchCapability".as_ptr() },
            PropertyEntry { name: c"horizontalSizeClass".as_ptr(), attributes: c"Tq,R,N,V_horizontalSizeClass".as_ptr() },
            PropertyEntry { name: c"userInterfaceIdiom".as_ptr(), attributes: c"Tq,R,N,V_userInterfaceIdiom".as_ptr() },
            PropertyEntry { name: c"userInterfaceStyle".as_ptr(), attributes: c"Tq,R,N,V_userInterfaceStyle".as_ptr() },
            PropertyEntry { name: c"verticalSizeClass".as_ptr(), attributes: c"Tq,R,N,V_verticalSizeClass".as_ptr() },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 8,
        instance_size: 56,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UITraitCollection".as_ptr(),
        methods: METHODS.ptr(),
        protocols: std::ptr::null(),
        ivars: IVARS.ptr(),
        weak_ivar_layout: std::ptr::null(),
        properties: PROPERTIES.ptr(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UITraitCollection".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UITraitCollection"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: &raw const NSObject_META,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UITraitCollection"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: &raw const NSObject_CLASS,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIResponder : NSObject -----------------------------------------
pub mod UIResponder {
    use super::*;

    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_canBecomeFirstResponder: i32 = 8;
    pub struct I_canBecomeFirstResponder;
    impl IvarOffset for I_canBecomeFirstResponder {
        fn offset() -> *const i32 {
            &raw const OFFSET_canBecomeFirstResponder
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_canResignFirstResponder: i32 = 9;
    pub struct I_canResignFirstResponder;
    impl IvarOffset for I_canResignFirstResponder {
        fn offset() -> *const i32 {
            &raw const OFFSET_canResignFirstResponder
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_isFirstResponder: i32 = 10;
    pub struct I_isFirstResponder;
    impl IvarOffset for I_isFirstResponder {
        fn offset() -> *const i32 {
            &raw const OFFSET_isFirstResponder
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_nextResponder: i32 = 16;
    pub struct I_nextResponder;
    impl IvarOffset for I_nextResponder {
        fn offset() -> *const i32 {
            &raw const OFFSET_nextResponder
        }
    }
    unsafe extern "C" fn cxx_destruct(this: Id, _: Sel) {
        destruct(this, &[&raw const OFFSET_nextResponder], &[]);
    }

    #[link_section = "__DATA,__objc_const"]
    static METHODS: Writable<MethodList<5>> = Writable::new(MethodList {
        entsize_flags: 24,
        count: 5,
        methods: [
            MethodEntry { name: c"nextResponder".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_value::<Id, I_nextResponder> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"canBecomeFirstResponder".as_ptr(), types: c"B16@0:8".as_ptr(), imp: (get_value::<bool, I_canBecomeFirstResponder> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"canResignFirstResponder".as_ptr(), types: c"B16@0:8".as_ptr(), imp: (get_value::<bool, I_canResignFirstResponder> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"isFirstResponder".as_ptr(), types: c"B16@0:8".as_ptr(), imp: (get_value::<bool, I_isFirstResponder> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c".cxx_destruct".as_ptr(), types: c"v16@0:8".as_ptr(), imp: (cxx_destruct) as *const c_void },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static IVARS: Writable<IvarList<4>> = Writable::new(IvarList {
        entsize: 32,
        count: 4,
        ivars: [
            IvarEntry { offset: &raw mut OFFSET_canBecomeFirstResponder, name: c"_canBecomeFirstResponder".as_ptr(), kind: c"B".as_ptr(), alignment_raw: 0, size: 1 },
            IvarEntry { offset: &raw mut OFFSET_canResignFirstResponder, name: c"_canResignFirstResponder".as_ptr(), kind: c"B".as_ptr(), alignment_raw: 0, size: 1 },
            IvarEntry { offset: &raw mut OFFSET_isFirstResponder, name: c"_isFirstResponder".as_ptr(), kind: c"B".as_ptr(), alignment_raw: 0, size: 1 },
            IvarEntry { offset: &raw mut OFFSET_nextResponder, name: c"_nextResponder".as_ptr(), kind: c"@\"UIResponder\"".as_ptr(), alignment_raw: 3, size: 8 },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static PROPERTIES: Writable<PropertyList<4>> = Writable::new(PropertyList {
        entsize: 16,
        count: 4,
        properties: [
            PropertyEntry { name: c"canBecomeFirstResponder".as_ptr(), attributes: c"TB,R,N,V_canBecomeFirstResponder".as_ptr() },
            PropertyEntry { name: c"canResignFirstResponder".as_ptr(), attributes: c"TB,R,N,V_canResignFirstResponder".as_ptr() },
            PropertyEntry { name: c"isFirstResponder".as_ptr(), attributes: c"TB,R,N,V_isFirstResponder".as_ptr() },
            PropertyEntry { name: c"nextResponder".as_ptr(), attributes: c"T@\"UIResponder\",R,N,V_nextResponder".as_ptr() },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x184,
        instance_start: 8,
        instance_size: 24,
        reserved: 0,
        ivar_layout: b"\x11\0".as_ptr(),
        name: c"UIResponder".as_ptr(),
        methods: METHODS.ptr(),
        protocols: std::ptr::null(),
        ivars: IVARS.ptr(),
        weak_ivar_layout: std::ptr::null(),
        properties: PROPERTIES.ptr(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x185,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIResponder".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIResponder"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: &raw const NSObject_META,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIResponder"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: &raw const NSObject_CLASS,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIScreen : NSObject --------------------------------------------
pub mod UIScreen {
    use super::*;

    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_scale: i32 = 8;
    pub struct I_scale;
    impl IvarOffset for I_scale {
        fn offset() -> *const i32 {
            &raw const OFFSET_scale
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_nativeScale: i32 = 16;
    pub struct I_nativeScale;
    impl IvarOffset for I_nativeScale {
        fn offset() -> *const i32 {
            &raw const OFFSET_nativeScale
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_maximumFramesPerSecond: i32 = 24;
    pub struct I_maximumFramesPerSecond;
    impl IvarOffset for I_maximumFramesPerSecond {
        fn offset() -> *const i32 {
            &raw const OFFSET_maximumFramesPerSecond
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_brightness: i32 = 32;
    pub struct I_brightness;
    impl IvarOffset for I_brightness {
        fn offset() -> *const i32 {
            &raw const OFFSET_brightness
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_traitCollection: i32 = 40;
    pub struct I_traitCollection;
    impl IvarOffset for I_traitCollection {
        fn offset() -> *const i32 {
            &raw const OFFSET_traitCollection
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_bounds: i32 = 48;
    pub struct I_bounds;
    impl IvarOffset for I_bounds {
        fn offset() -> *const i32 {
            &raw const OFFSET_bounds
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_nativeBounds: i32 = 80;
    pub struct I_nativeBounds;
    impl IvarOffset for I_nativeBounds {
        fn offset() -> *const i32 {
            &raw const OFFSET_nativeBounds
        }
    }
    unsafe extern "C" fn cxx_destruct(this: Id, _: Sel) {
        destruct(this, &[&raw const OFFSET_traitCollection], &[]);
    }

    #[link_section = "__DATA,__objc_const"]
    static METHODS: Writable<MethodList<9>> = Writable::new(MethodList {
        entsize_flags: 24,
        count: 9,
        methods: [
            MethodEntry { name: c"bounds".as_ptr(), types: c"{CGRect={CGPoint=dd}{CGSize=dd}}16@0:8".as_ptr(), imp: (get_value::<CGRect, I_bounds> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"nativeBounds".as_ptr(), types: c"{CGRect={CGPoint=dd}{CGSize=dd}}16@0:8".as_ptr(), imp: (get_value::<CGRect, I_nativeBounds> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"scale".as_ptr(), types: c"d16@0:8".as_ptr(), imp: (get_value::<f64, I_scale> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"nativeScale".as_ptr(), types: c"d16@0:8".as_ptr(), imp: (get_value::<f64, I_nativeScale> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"maximumFramesPerSecond".as_ptr(), types: c"q16@0:8".as_ptr(), imp: (get_value::<isize, I_maximumFramesPerSecond> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"brightness".as_ptr(), types: c"d16@0:8".as_ptr(), imp: (get_value::<f64, I_brightness> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setBrightness:".as_ptr(), types: c"v24@0:8d16".as_ptr(), imp: (set_value::<f64, I_brightness> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c"traitCollection".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_value::<Id, I_traitCollection> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c".cxx_destruct".as_ptr(), types: c"v16@0:8".as_ptr(), imp: (cxx_destruct) as *const c_void },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static IVARS: Writable<IvarList<7>> = Writable::new(IvarList {
        entsize: 32,
        count: 7,
        ivars: [
            IvarEntry { offset: &raw mut OFFSET_scale, name: c"_scale".as_ptr(), kind: c"d".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_nativeScale, name: c"_nativeScale".as_ptr(), kind: c"d".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_maximumFramesPerSecond, name: c"_maximumFramesPerSecond".as_ptr(), kind: c"q".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_brightness, name: c"_brightness".as_ptr(), kind: c"d".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_traitCollection, name: c"_traitCollection".as_ptr(), kind: c"@\"UITraitCollection\"".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_bounds, name: c"_bounds".as_ptr(), kind: c"{CGRect=\"origin\"{CGPoint=\"x\"d\"y\"d}\"size\"{CGSize=\"width\"d\"height\"d}}".as_ptr(), alignment_raw: 3, size: 32 },
            IvarEntry { offset: &raw mut OFFSET_nativeBounds, name: c"_nativeBounds".as_ptr(), kind: c"{CGRect=\"origin\"{CGPoint=\"x\"d\"y\"d}\"size\"{CGSize=\"width\"d\"height\"d}}".as_ptr(), alignment_raw: 3, size: 32 },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static PROPERTIES: Writable<PropertyList<7>> = Writable::new(PropertyList {
        entsize: 16,
        count: 7,
        properties: [
            PropertyEntry { name: c"bounds".as_ptr(), attributes: c"T{CGRect={CGPoint=dd}{CGSize=dd}},R,N,V_bounds".as_ptr() },
            PropertyEntry { name: c"brightness".as_ptr(), attributes: c"Td,N,V_brightness".as_ptr() },
            PropertyEntry { name: c"maximumFramesPerSecond".as_ptr(), attributes: c"Tq,R,N,V_maximumFramesPerSecond".as_ptr() },
            PropertyEntry { name: c"nativeBounds".as_ptr(), attributes: c"T{CGRect={CGPoint=dd}{CGSize=dd}},R,N,V_nativeBounds".as_ptr() },
            PropertyEntry { name: c"nativeScale".as_ptr(), attributes: c"Td,R,N,V_nativeScale".as_ptr() },
            PropertyEntry { name: c"scale".as_ptr(), attributes: c"Td,R,N,V_scale".as_ptr() },
            PropertyEntry { name: c"traitCollection".as_ptr(), attributes: c"T@\"UITraitCollection\",R,N,V_traitCollection".as_ptr() },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x184,
        instance_start: 8,
        instance_size: 112,
        reserved: 0,
        ivar_layout: b"\x41\0".as_ptr(),
        name: c"UIScreen".as_ptr(),
        methods: METHODS.ptr(),
        protocols: std::ptr::null(),
        ivars: IVARS.ptr(),
        weak_ivar_layout: std::ptr::null(),
        properties: PROPERTIES.ptr(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x185,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIScreen".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIScreen"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: &raw const NSObject_META,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIScreen"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: &raw const NSObject_CLASS,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIDevice : NSObject --------------------------------------------
pub mod UIDevice {
    use super::*;

    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_batteryMonitoringEnabled: i32 = 8;
    pub struct I_batteryMonitoringEnabled;
    impl IvarOffset for I_batteryMonitoringEnabled {
        fn offset() -> *const i32 {
            &raw const OFFSET_batteryMonitoringEnabled
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_multitaskingSupported: i32 = 9;
    pub struct I_multitaskingSupported;
    impl IvarOffset for I_multitaskingSupported {
        fn offset() -> *const i32 {
            &raw const OFFSET_multitaskingSupported
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_batteryLevel: i32 = 12;
    pub struct I_batteryLevel;
    impl IvarOffset for I_batteryLevel {
        fn offset() -> *const i32 {
            &raw const OFFSET_batteryLevel
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_name: i32 = 16;
    pub struct I_name;
    impl IvarOffset for I_name {
        fn offset() -> *const i32 {
            &raw const OFFSET_name
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_model: i32 = 24;
    pub struct I_model;
    impl IvarOffset for I_model {
        fn offset() -> *const i32 {
            &raw const OFFSET_model
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_localizedModel: i32 = 32;
    pub struct I_localizedModel;
    impl IvarOffset for I_localizedModel {
        fn offset() -> *const i32 {
            &raw const OFFSET_localizedModel
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_systemName: i32 = 40;
    pub struct I_systemName;
    impl IvarOffset for I_systemName {
        fn offset() -> *const i32 {
            &raw const OFFSET_systemName
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_systemVersion: i32 = 48;
    pub struct I_systemVersion;
    impl IvarOffset for I_systemVersion {
        fn offset() -> *const i32 {
            &raw const OFFSET_systemVersion
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_orientation: i32 = 56;
    pub struct I_orientation;
    impl IvarOffset for I_orientation {
        fn offset() -> *const i32 {
            &raw const OFFSET_orientation
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_identifierForVendor: i32 = 64;
    pub struct I_identifierForVendor;
    impl IvarOffset for I_identifierForVendor {
        fn offset() -> *const i32 {
            &raw const OFFSET_identifierForVendor
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_userInterfaceIdiom: i32 = 72;
    pub struct I_userInterfaceIdiom;
    impl IvarOffset for I_userInterfaceIdiom {
        fn offset() -> *const i32 {
            &raw const OFFSET_userInterfaceIdiom
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_batteryState: i32 = 80;
    pub struct I_batteryState;
    impl IvarOffset for I_batteryState {
        fn offset() -> *const i32 {
            &raw const OFFSET_batteryState
        }
    }
    unsafe extern "C" fn cxx_destruct(this: Id, _: Sel) {
        destruct(this, &[&raw const OFFSET_name, &raw const OFFSET_model, &raw const OFFSET_localizedModel, &raw const OFFSET_systemName, &raw const OFFSET_systemVersion, &raw const OFFSET_identifierForVendor], &[]);
    }

    #[link_section = "__DATA,__objc_const"]
    static METHODS: Writable<MethodList<14>> = Writable::new(MethodList {
        entsize_flags: 24,
        count: 14,
        methods: [
            MethodEntry { name: c"name".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_value::<Id, I_name> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"model".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_value::<Id, I_model> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"localizedModel".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_value::<Id, I_localizedModel> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"systemName".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_value::<Id, I_systemName> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"systemVersion".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_value::<Id, I_systemVersion> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"orientation".as_ptr(), types: c"q16@0:8".as_ptr(), imp: (get_value::<isize, I_orientation> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"identifierForVendor".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_value::<Id, I_identifierForVendor> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"userInterfaceIdiom".as_ptr(), types: c"q16@0:8".as_ptr(), imp: (get_value::<isize, I_userInterfaceIdiom> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"isBatteryMonitoringEnabled".as_ptr(), types: c"B16@0:8".as_ptr(), imp: (get_value::<bool, I_batteryMonitoringEnabled> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setBatteryMonitoringEnabled:".as_ptr(), types: c"v20@0:8B16".as_ptr(), imp: (set_value::<bool, I_batteryMonitoringEnabled> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c"batteryState".as_ptr(), types: c"q16@0:8".as_ptr(), imp: (get_value::<isize, I_batteryState> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"batteryLevel".as_ptr(), types: c"f16@0:8".as_ptr(), imp: (get_value::<f32, I_batteryLevel> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"isMultitaskingSupported".as_ptr(), types: c"B16@0:8".as_ptr(), imp: (get_value::<bool, I_multitaskingSupported> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c".cxx_destruct".as_ptr(), types: c"v16@0:8".as_ptr(), imp: (cxx_destruct) as *const c_void },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static IVARS: Writable<IvarList<12>> = Writable::new(IvarList {
        entsize: 32,
        count: 12,
        ivars: [
            IvarEntry { offset: &raw mut OFFSET_batteryMonitoringEnabled, name: c"_batteryMonitoringEnabled".as_ptr(), kind: c"B".as_ptr(), alignment_raw: 0, size: 1 },
            IvarEntry { offset: &raw mut OFFSET_multitaskingSupported, name: c"_multitaskingSupported".as_ptr(), kind: c"B".as_ptr(), alignment_raw: 0, size: 1 },
            IvarEntry { offset: &raw mut OFFSET_batteryLevel, name: c"_batteryLevel".as_ptr(), kind: c"f".as_ptr(), alignment_raw: 2, size: 4 },
            IvarEntry { offset: &raw mut OFFSET_name, name: c"_name".as_ptr(), kind: c"@\"NSString\"".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_model, name: c"_model".as_ptr(), kind: c"@\"NSString\"".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_localizedModel, name: c"_localizedModel".as_ptr(), kind: c"@\"NSString\"".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_systemName, name: c"_systemName".as_ptr(), kind: c"@\"NSString\"".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_systemVersion, name: c"_systemVersion".as_ptr(), kind: c"@\"NSString\"".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_orientation, name: c"_orientation".as_ptr(), kind: c"q".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_identifierForVendor, name: c"_identifierForVendor".as_ptr(), kind: c"@\"NSUUID\"".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_userInterfaceIdiom, name: c"_userInterfaceIdiom".as_ptr(), kind: c"q".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_batteryState, name: c"_batteryState".as_ptr(), kind: c"q".as_ptr(), alignment_raw: 3, size: 8 },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static PROPERTIES: Writable<PropertyList<12>> = Writable::new(PropertyList {
        entsize: 16,
        count: 12,
        properties: [
            PropertyEntry { name: c"batteryLevel".as_ptr(), attributes: c"Tf,R,N,V_batteryLevel".as_ptr() },
            PropertyEntry { name: c"batteryMonitoringEnabled".as_ptr(), attributes: c"TB,N,GisBatteryMonitoringEnabled,V_batteryMonitoringEnabled".as_ptr() },
            PropertyEntry { name: c"batteryState".as_ptr(), attributes: c"Tq,R,N,V_batteryState".as_ptr() },
            PropertyEntry { name: c"identifierForVendor".as_ptr(), attributes: c"T@\"NSUUID\",R,N,V_identifierForVendor".as_ptr() },
            PropertyEntry { name: c"localizedModel".as_ptr(), attributes: c"T@\"NSString\",R,N,V_localizedModel".as_ptr() },
            PropertyEntry { name: c"model".as_ptr(), attributes: c"T@\"NSString\",R,N,V_model".as_ptr() },
            PropertyEntry { name: c"multitaskingSupported".as_ptr(), attributes: c"TB,R,N,GisMultitaskingSupported,V_multitaskingSupported".as_ptr() },
            PropertyEntry { name: c"name".as_ptr(), attributes: c"T@\"NSString\",R,N,V_name".as_ptr() },
            PropertyEntry { name: c"orientation".as_ptr(), attributes: c"Tq,R,N,V_orientation".as_ptr() },
            PropertyEntry { name: c"systemName".as_ptr(), attributes: c"T@\"NSString\",R,N,V_systemName".as_ptr() },
            PropertyEntry { name: c"systemVersion".as_ptr(), attributes: c"T@\"NSString\",R,N,V_systemVersion".as_ptr() },
            PropertyEntry { name: c"userInterfaceIdiom".as_ptr(), attributes: c"Tq,R,N,V_userInterfaceIdiom".as_ptr() },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x184,
        instance_start: 8,
        instance_size: 88,
        reserved: 0,
        ivar_layout: b"\x15\x11\0".as_ptr(),
        name: c"UIDevice".as_ptr(),
        methods: METHODS.ptr(),
        protocols: std::ptr::null(),
        ivars: IVARS.ptr(),
        weak_ivar_layout: std::ptr::null(),
        properties: PROPERTIES.ptr(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x185,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIDevice".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIDevice"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: &raw const NSObject_META,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIDevice"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: &raw const NSObject_CLASS,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIView : UIResponder -------------------------------------------
pub mod UIView {
    use super::*;

    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_multipleTouchEnabled: i32 = 24;
    pub struct I_multipleTouchEnabled;
    impl IvarOffset for I_multipleTouchEnabled {
        fn offset() -> *const i32 {
            &raw const OFFSET_multipleTouchEnabled
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_userInteractionEnabled: i32 = 25;
    pub struct I_userInteractionEnabled;
    impl IvarOffset for I_userInteractionEnabled {
        fn offset() -> *const i32 {
            &raw const OFFSET_userInteractionEnabled
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_hidden: i32 = 26;
    pub struct I_hidden;
    impl IvarOffset for I_hidden {
        fn offset() -> *const i32 {
            &raw const OFFSET_hidden
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_opaque: i32 = 27;
    pub struct I_opaque;
    impl IvarOffset for I_opaque {
        fn offset() -> *const i32 {
            &raw const OFFSET_opaque
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_clearsContextBeforeDrawing: i32 = 28;
    pub struct I_clearsContextBeforeDrawing;
    impl IvarOffset for I_clearsContextBeforeDrawing {
        fn offset() -> *const i32 {
            &raw const OFFSET_clearsContextBeforeDrawing
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_layer: i32 = 32;
    pub struct I_layer;
    impl IvarOffset for I_layer {
        fn offset() -> *const i32 {
            &raw const OFFSET_layer
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_superview: i32 = 40;
    pub struct I_superview;
    impl IvarOffset for I_superview {
        fn offset() -> *const i32 {
            &raw const OFFSET_superview
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_subviews: i32 = 48;
    pub struct I_subviews;
    impl IvarOffset for I_subviews {
        fn offset() -> *const i32 {
            &raw const OFFSET_subviews
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_window: i32 = 56;
    pub struct I_window;
    impl IvarOffset for I_window {
        fn offset() -> *const i32 {
            &raw const OFFSET_window
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_contentScaleFactor: i32 = 64;
    pub struct I_contentScaleFactor;
    impl IvarOffset for I_contentScaleFactor {
        fn offset() -> *const i32 {
            &raw const OFFSET_contentScaleFactor
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_alpha: i32 = 72;
    pub struct I_alpha;
    impl IvarOffset for I_alpha {
        fn offset() -> *const i32 {
            &raw const OFFSET_alpha
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_tag: i32 = 80;
    pub struct I_tag;
    impl IvarOffset for I_tag {
        fn offset() -> *const i32 {
            &raw const OFFSET_tag
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_autoresizingMask: i32 = 88;
    pub struct I_autoresizingMask;
    impl IvarOffset for I_autoresizingMask {
        fn offset() -> *const i32 {
            &raw const OFFSET_autoresizingMask
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_backgroundColor: i32 = 96;
    pub struct I_backgroundColor;
    impl IvarOffset for I_backgroundColor {
        fn offset() -> *const i32 {
            &raw const OFFSET_backgroundColor
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_traitCollection: i32 = 104;
    pub struct I_traitCollection;
    impl IvarOffset for I_traitCollection {
        fn offset() -> *const i32 {
            &raw const OFFSET_traitCollection
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_center: i32 = 112;
    pub struct I_center;
    impl IvarOffset for I_center {
        fn offset() -> *const i32 {
            &raw const OFFSET_center
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_frame: i32 = 128;
    pub struct I_frame;
    impl IvarOffset for I_frame {
        fn offset() -> *const i32 {
            &raw const OFFSET_frame
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_bounds: i32 = 160;
    pub struct I_bounds;
    impl IvarOffset for I_bounds {
        fn offset() -> *const i32 {
            &raw const OFFSET_bounds
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_safeAreaInsets: i32 = 192;
    pub struct I_safeAreaInsets;
    impl IvarOffset for I_safeAreaInsets {
        fn offset() -> *const i32 {
            &raw const OFFSET_safeAreaInsets
        }
    }
    unsafe extern "C" fn cxx_destruct(this: Id, _: Sel) {
        destruct(this, &[&raw const OFFSET_layer, &raw const OFFSET_superview, &raw const OFFSET_subviews, &raw const OFFSET_window, &raw const OFFSET_backgroundColor, &raw const OFFSET_traitCollection], &[]);
    }

    #[link_section = "__DATA,__objc_const"]
    static METHODS: Writable<MethodList<33>> = Writable::new(MethodList {
        entsize_flags: 24,
        count: 33,
        methods: [
            MethodEntry { name: c"layer".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_value::<Id, I_layer> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"frame".as_ptr(), types: c"{CGRect={CGPoint=dd}{CGSize=dd}}16@0:8".as_ptr(), imp: (get_value::<CGRect, I_frame> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setFrame:".as_ptr(), types: c"v48@0:8{CGRect={CGPoint=dd}{CGSize=dd}}16".as_ptr(), imp: (set_value::<CGRect, I_frame> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c"bounds".as_ptr(), types: c"{CGRect={CGPoint=dd}{CGSize=dd}}16@0:8".as_ptr(), imp: (get_value::<CGRect, I_bounds> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setBounds:".as_ptr(), types: c"v48@0:8{CGRect={CGPoint=dd}{CGSize=dd}}16".as_ptr(), imp: (set_value::<CGRect, I_bounds> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c"center".as_ptr(), types: c"{CGPoint=dd}16@0:8".as_ptr(), imp: (get_value::<CGPoint, I_center> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setCenter:".as_ptr(), types: c"v32@0:8{CGPoint=dd}16".as_ptr(), imp: (set_value::<CGPoint, I_center> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c"superview".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_value::<Id, I_superview> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"subviews".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_value::<Id, I_subviews> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"window".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_value::<Id, I_window> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"contentScaleFactor".as_ptr(), types: c"d16@0:8".as_ptr(), imp: (get_value::<f64, I_contentScaleFactor> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setContentScaleFactor:".as_ptr(), types: c"v24@0:8d16".as_ptr(), imp: (set_value::<f64, I_contentScaleFactor> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c"isMultipleTouchEnabled".as_ptr(), types: c"B16@0:8".as_ptr(), imp: (get_value::<bool, I_multipleTouchEnabled> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setMultipleTouchEnabled:".as_ptr(), types: c"v20@0:8B16".as_ptr(), imp: (set_value::<bool, I_multipleTouchEnabled> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c"isUserInteractionEnabled".as_ptr(), types: c"B16@0:8".as_ptr(), imp: (get_value::<bool, I_userInteractionEnabled> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setUserInteractionEnabled:".as_ptr(), types: c"v20@0:8B16".as_ptr(), imp: (set_value::<bool, I_userInteractionEnabled> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c"isHidden".as_ptr(), types: c"B16@0:8".as_ptr(), imp: (get_value::<bool, I_hidden> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setHidden:".as_ptr(), types: c"v20@0:8B16".as_ptr(), imp: (set_value::<bool, I_hidden> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c"isOpaque".as_ptr(), types: c"B16@0:8".as_ptr(), imp: (get_value::<bool, I_opaque> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setOpaque:".as_ptr(), types: c"v20@0:8B16".as_ptr(), imp: (set_value::<bool, I_opaque> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c"alpha".as_ptr(), types: c"d16@0:8".as_ptr(), imp: (get_value::<f64, I_alpha> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setAlpha:".as_ptr(), types: c"v24@0:8d16".as_ptr(), imp: (set_value::<f64, I_alpha> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c"tag".as_ptr(), types: c"q16@0:8".as_ptr(), imp: (get_value::<isize, I_tag> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setTag:".as_ptr(), types: c"v24@0:8q16".as_ptr(), imp: (set_value::<isize, I_tag> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c"autoresizingMask".as_ptr(), types: c"Q16@0:8".as_ptr(), imp: (get_value::<usize, I_autoresizingMask> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setAutoresizingMask:".as_ptr(), types: c"v24@0:8Q16".as_ptr(), imp: (set_value::<usize, I_autoresizingMask> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c"clearsContextBeforeDrawing".as_ptr(), types: c"B16@0:8".as_ptr(), imp: (get_value::<bool, I_clearsContextBeforeDrawing> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setClearsContextBeforeDrawing:".as_ptr(), types: c"v20@0:8B16".as_ptr(), imp: (set_value::<bool, I_clearsContextBeforeDrawing> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c"backgroundColor".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_value::<Id, I_backgroundColor> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setBackgroundColor:".as_ptr(), types: c"v24@0:8@16".as_ptr(), imp: (set_copy::<I_backgroundColor> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c"safeAreaInsets".as_ptr(), types: c"{UIEdgeInsets=dddd}16@0:8".as_ptr(), imp: (get_value::<UIEdgeInsets, I_safeAreaInsets> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"traitCollection".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_value::<Id, I_traitCollection> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c".cxx_destruct".as_ptr(), types: c"v16@0:8".as_ptr(), imp: (cxx_destruct) as *const c_void },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static IVARS: Writable<IvarList<19>> = Writable::new(IvarList {
        entsize: 32,
        count: 19,
        ivars: [
            IvarEntry { offset: &raw mut OFFSET_multipleTouchEnabled, name: c"_multipleTouchEnabled".as_ptr(), kind: c"B".as_ptr(), alignment_raw: 0, size: 1 },
            IvarEntry { offset: &raw mut OFFSET_userInteractionEnabled, name: c"_userInteractionEnabled".as_ptr(), kind: c"B".as_ptr(), alignment_raw: 0, size: 1 },
            IvarEntry { offset: &raw mut OFFSET_hidden, name: c"_hidden".as_ptr(), kind: c"B".as_ptr(), alignment_raw: 0, size: 1 },
            IvarEntry { offset: &raw mut OFFSET_opaque, name: c"_opaque".as_ptr(), kind: c"B".as_ptr(), alignment_raw: 0, size: 1 },
            IvarEntry { offset: &raw mut OFFSET_clearsContextBeforeDrawing, name: c"_clearsContextBeforeDrawing".as_ptr(), kind: c"B".as_ptr(), alignment_raw: 0, size: 1 },
            IvarEntry { offset: &raw mut OFFSET_layer, name: c"_layer".as_ptr(), kind: c"@\"CALayer\"".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_superview, name: c"_superview".as_ptr(), kind: c"@\"UIView\"".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_subviews, name: c"_subviews".as_ptr(), kind: c"@\"NSArray\"".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_window, name: c"_window".as_ptr(), kind: c"@\"UIWindow\"".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_contentScaleFactor, name: c"_contentScaleFactor".as_ptr(), kind: c"d".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_alpha, name: c"_alpha".as_ptr(), kind: c"d".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_tag, name: c"_tag".as_ptr(), kind: c"q".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_autoresizingMask, name: c"_autoresizingMask".as_ptr(), kind: c"Q".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_backgroundColor, name: c"_backgroundColor".as_ptr(), kind: c"@\"UIColor\"".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_traitCollection, name: c"_traitCollection".as_ptr(), kind: c"@\"UITraitCollection\"".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_center, name: c"_center".as_ptr(), kind: c"{CGPoint=\"x\"d\"y\"d}".as_ptr(), alignment_raw: 3, size: 16 },
            IvarEntry { offset: &raw mut OFFSET_frame, name: c"_frame".as_ptr(), kind: c"{CGRect=\"origin\"{CGPoint=\"x\"d\"y\"d}\"size\"{CGSize=\"width\"d\"height\"d}}".as_ptr(), alignment_raw: 3, size: 32 },
            IvarEntry { offset: &raw mut OFFSET_bounds, name: c"_bounds".as_ptr(), kind: c"{CGRect=\"origin\"{CGPoint=\"x\"d\"y\"d}\"size\"{CGSize=\"width\"d\"height\"d}}".as_ptr(), alignment_raw: 3, size: 32 },
            IvarEntry { offset: &raw mut OFFSET_safeAreaInsets, name: c"_safeAreaInsets".as_ptr(), kind: c"{UIEdgeInsets=\"top\"d\"left\"d\"bottom\"d\"right\"d}".as_ptr(), alignment_raw: 3, size: 32 },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static PROPERTIES: Writable<PropertyList<19>> = Writable::new(PropertyList {
        entsize: 16,
        count: 19,
        properties: [
            PropertyEntry { name: c"alpha".as_ptr(), attributes: c"Td,N,V_alpha".as_ptr() },
            PropertyEntry { name: c"autoresizingMask".as_ptr(), attributes: c"TQ,N,V_autoresizingMask".as_ptr() },
            PropertyEntry { name: c"backgroundColor".as_ptr(), attributes: c"T@\"UIColor\",C,N,V_backgroundColor".as_ptr() },
            PropertyEntry { name: c"bounds".as_ptr(), attributes: c"T{CGRect={CGPoint=dd}{CGSize=dd}},N,V_bounds".as_ptr() },
            PropertyEntry { name: c"center".as_ptr(), attributes: c"T{CGPoint=dd},N,V_center".as_ptr() },
            PropertyEntry { name: c"clearsContextBeforeDrawing".as_ptr(), attributes: c"TB,N,V_clearsContextBeforeDrawing".as_ptr() },
            PropertyEntry { name: c"contentScaleFactor".as_ptr(), attributes: c"Td,N,V_contentScaleFactor".as_ptr() },
            PropertyEntry { name: c"frame".as_ptr(), attributes: c"T{CGRect={CGPoint=dd}{CGSize=dd}},N,V_frame".as_ptr() },
            PropertyEntry { name: c"hidden".as_ptr(), attributes: c"TB,N,GisHidden,V_hidden".as_ptr() },
            PropertyEntry { name: c"layer".as_ptr(), attributes: c"T@\"CALayer\",R,N,V_layer".as_ptr() },
            PropertyEntry { name: c"multipleTouchEnabled".as_ptr(), attributes: c"TB,N,GisMultipleTouchEnabled,V_multipleTouchEnabled".as_ptr() },
            PropertyEntry { name: c"opaque".as_ptr(), attributes: c"TB,N,GisOpaque,V_opaque".as_ptr() },
            PropertyEntry { name: c"safeAreaInsets".as_ptr(), attributes: c"T{UIEdgeInsets=dddd},R,N,V_safeAreaInsets".as_ptr() },
            PropertyEntry { name: c"subviews".as_ptr(), attributes: c"T@\"NSArray\",R,N,V_subviews".as_ptr() },
            PropertyEntry { name: c"superview".as_ptr(), attributes: c"T@\"UIView\",R,N,V_superview".as_ptr() },
            PropertyEntry { name: c"tag".as_ptr(), attributes: c"Tq,N,V_tag".as_ptr() },
            PropertyEntry { name: c"traitCollection".as_ptr(), attributes: c"T@\"UITraitCollection\",R,N,V_traitCollection".as_ptr() },
            PropertyEntry { name: c"userInteractionEnabled".as_ptr(), attributes: c"TB,N,GisUserInteractionEnabled,V_userInteractionEnabled".as_ptr() },
            PropertyEntry { name: c"window".as_ptr(), attributes: c"T@\"UIWindow\",R,N,V_window".as_ptr() },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x184,
        instance_start: 24,
        instance_size: 224,
        reserved: 0,
        ivar_layout: b"\x14\x42\0".as_ptr(),
        name: c"UIView".as_ptr(),
        methods: METHODS.ptr(),
        protocols: std::ptr::null(),
        ivars: IVARS.ptr(),
        weak_ivar_layout: std::ptr::null(),
        properties: PROPERTIES.ptr(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x185,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIView".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIView"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: super::UIResponder::META.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIView"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: super::UIResponder::CLASS.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIViewController : UIResponder ---------------------------------
pub mod UIViewController {
    use super::*;

    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_viewLoaded: i32 = 24;
    pub struct I_viewLoaded;
    impl IvarOffset for I_viewLoaded {
        fn offset() -> *const i32 {
            &raw const OFFSET_viewLoaded
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_view: i32 = 32;
    pub struct I_view;
    impl IvarOffset for I_view {
        fn offset() -> *const i32 {
            &raw const OFFSET_view
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_viewIfLoaded: i32 = 40;
    pub struct I_viewIfLoaded;
    impl IvarOffset for I_viewIfLoaded {
        fn offset() -> *const i32 {
            &raw const OFFSET_viewIfLoaded
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_title: i32 = 48;
    pub struct I_title;
    impl IvarOffset for I_title {
        fn offset() -> *const i32 {
            &raw const OFFSET_title
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_modalPresentationStyle: i32 = 56;
    pub struct I_modalPresentationStyle;
    impl IvarOffset for I_modalPresentationStyle {
        fn offset() -> *const i32 {
            &raw const OFFSET_modalPresentationStyle
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_edgesForExtendedLayout: i32 = 64;
    pub struct I_edgesForExtendedLayout;
    impl IvarOffset for I_edgesForExtendedLayout {
        fn offset() -> *const i32 {
            &raw const OFFSET_edgesForExtendedLayout
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_presentedViewController: i32 = 72;
    pub struct I_presentedViewController;
    impl IvarOffset for I_presentedViewController {
        fn offset() -> *const i32 {
            &raw const OFFSET_presentedViewController
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_presentingViewController: i32 = 80;
    pub struct I_presentingViewController;
    impl IvarOffset for I_presentingViewController {
        fn offset() -> *const i32 {
            &raw const OFFSET_presentingViewController
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_traitCollection: i32 = 88;
    pub struct I_traitCollection;
    impl IvarOffset for I_traitCollection {
        fn offset() -> *const i32 {
            &raw const OFFSET_traitCollection
        }
    }
    unsafe extern "C" fn cxx_destruct(this: Id, _: Sel) {
        destruct(this, &[&raw const OFFSET_view, &raw const OFFSET_viewIfLoaded, &raw const OFFSET_title, &raw const OFFSET_presentedViewController, &raw const OFFSET_presentingViewController, &raw const OFFSET_traitCollection], &[]);
    }

    #[link_section = "__DATA,__objc_const"]
    static METHODS: Writable<MethodList<14>> = Writable::new(MethodList {
        entsize_flags: 24,
        count: 14,
        methods: [
            MethodEntry { name: c"view".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_value::<Id, I_view> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setView:".as_ptr(), types: c"v24@0:8@16".as_ptr(), imp: (set_strong::<I_view> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c"viewIfLoaded".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_value::<Id, I_viewIfLoaded> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"isViewLoaded".as_ptr(), types: c"B16@0:8".as_ptr(), imp: (get_value::<bool, I_viewLoaded> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"title".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_value::<Id, I_title> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setTitle:".as_ptr(), types: c"v24@0:8@16".as_ptr(), imp: (set_copy::<I_title> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c"modalPresentationStyle".as_ptr(), types: c"q16@0:8".as_ptr(), imp: (get_value::<isize, I_modalPresentationStyle> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setModalPresentationStyle:".as_ptr(), types: c"v24@0:8q16".as_ptr(), imp: (set_value::<isize, I_modalPresentationStyle> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c"edgesForExtendedLayout".as_ptr(), types: c"Q16@0:8".as_ptr(), imp: (get_value::<usize, I_edgesForExtendedLayout> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setEdgesForExtendedLayout:".as_ptr(), types: c"v24@0:8Q16".as_ptr(), imp: (set_value::<usize, I_edgesForExtendedLayout> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c"presentedViewController".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_value::<Id, I_presentedViewController> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"presentingViewController".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_value::<Id, I_presentingViewController> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"traitCollection".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_value::<Id, I_traitCollection> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c".cxx_destruct".as_ptr(), types: c"v16@0:8".as_ptr(), imp: (cxx_destruct) as *const c_void },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static IVARS: Writable<IvarList<9>> = Writable::new(IvarList {
        entsize: 32,
        count: 9,
        ivars: [
            IvarEntry { offset: &raw mut OFFSET_viewLoaded, name: c"_viewLoaded".as_ptr(), kind: c"B".as_ptr(), alignment_raw: 0, size: 1 },
            IvarEntry { offset: &raw mut OFFSET_view, name: c"_view".as_ptr(), kind: c"@\"UIView\"".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_viewIfLoaded, name: c"_viewIfLoaded".as_ptr(), kind: c"@\"UIView\"".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_title, name: c"_title".as_ptr(), kind: c"@\"NSString\"".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_modalPresentationStyle, name: c"_modalPresentationStyle".as_ptr(), kind: c"q".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_edgesForExtendedLayout, name: c"_edgesForExtendedLayout".as_ptr(), kind: c"Q".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_presentedViewController, name: c"_presentedViewController".as_ptr(), kind: c"@\"UIViewController\"".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_presentingViewController, name: c"_presentingViewController".as_ptr(), kind: c"@\"UIViewController\"".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_traitCollection, name: c"_traitCollection".as_ptr(), kind: c"@\"UITraitCollection\"".as_ptr(), alignment_raw: 3, size: 8 },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static PROPERTIES: Writable<PropertyList<9>> = Writable::new(PropertyList {
        entsize: 16,
        count: 9,
        properties: [
            PropertyEntry { name: c"edgesForExtendedLayout".as_ptr(), attributes: c"TQ,N,V_edgesForExtendedLayout".as_ptr() },
            PropertyEntry { name: c"modalPresentationStyle".as_ptr(), attributes: c"Tq,N,V_modalPresentationStyle".as_ptr() },
            PropertyEntry { name: c"presentedViewController".as_ptr(), attributes: c"T@\"UIViewController\",R,N,V_presentedViewController".as_ptr() },
            PropertyEntry { name: c"presentingViewController".as_ptr(), attributes: c"T@\"UIViewController\",R,N,V_presentingViewController".as_ptr() },
            PropertyEntry { name: c"title".as_ptr(), attributes: c"T@\"NSString\",C,N,V_title".as_ptr() },
            PropertyEntry { name: c"traitCollection".as_ptr(), attributes: c"T@\"UITraitCollection\",R,N,V_traitCollection".as_ptr() },
            PropertyEntry { name: c"view".as_ptr(), attributes: c"T@\"UIView\",&,N,V_view".as_ptr() },
            PropertyEntry { name: c"viewIfLoaded".as_ptr(), attributes: c"T@\"UIView\",R,N,V_viewIfLoaded".as_ptr() },
            PropertyEntry { name: c"viewLoaded".as_ptr(), attributes: c"TB,R,N,GisViewLoaded,V_viewLoaded".as_ptr() },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x184,
        instance_start: 24,
        instance_size: 96,
        reserved: 0,
        ivar_layout: b"\x13\x23\0".as_ptr(),
        name: c"UIViewController".as_ptr(),
        methods: METHODS.ptr(),
        protocols: std::ptr::null(),
        ivars: IVARS.ptr(),
        weak_ivar_layout: std::ptr::null(),
        properties: PROPERTIES.ptr(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x185,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIViewController".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIViewController"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: super::UIResponder::META.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIViewController"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: super::UIResponder::CLASS.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIWindow : UIView ----------------------------------------------
pub mod UIWindow {
    use super::*;

    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_keyWindow: i32 = 224;
    pub struct I_keyWindow;
    impl IvarOffset for I_keyWindow {
        fn offset() -> *const i32 {
            &raw const OFFSET_keyWindow
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_rootViewController: i32 = 232;
    pub struct I_rootViewController;
    impl IvarOffset for I_rootViewController {
        fn offset() -> *const i32 {
            &raw const OFFSET_rootViewController
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_screen: i32 = 240;
    pub struct I_screen;
    impl IvarOffset for I_screen {
        fn offset() -> *const i32 {
            &raw const OFFSET_screen
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_windowLevel: i32 = 248;
    pub struct I_windowLevel;
    impl IvarOffset for I_windowLevel {
        fn offset() -> *const i32 {
            &raw const OFFSET_windowLevel
        }
    }
    unsafe extern "C" fn cxx_destruct(this: Id, _: Sel) {
        destruct(this, &[&raw const OFFSET_rootViewController, &raw const OFFSET_screen], &[]);
    }

    #[link_section = "__DATA,__objc_const"]
    static METHODS: Writable<MethodList<8>> = Writable::new(MethodList {
        entsize_flags: 24,
        count: 8,
        methods: [
            MethodEntry { name: c"rootViewController".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_value::<Id, I_rootViewController> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setRootViewController:".as_ptr(), types: c"v24@0:8@16".as_ptr(), imp: (set_strong::<I_rootViewController> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c"screen".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_value::<Id, I_screen> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setScreen:".as_ptr(), types: c"v24@0:8@16".as_ptr(), imp: (set_strong::<I_screen> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c"isKeyWindow".as_ptr(), types: c"B16@0:8".as_ptr(), imp: (get_value::<bool, I_keyWindow> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"windowLevel".as_ptr(), types: c"d16@0:8".as_ptr(), imp: (get_value::<f64, I_windowLevel> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setWindowLevel:".as_ptr(), types: c"v24@0:8d16".as_ptr(), imp: (set_value::<f64, I_windowLevel> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c".cxx_destruct".as_ptr(), types: c"v16@0:8".as_ptr(), imp: (cxx_destruct) as *const c_void },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static IVARS: Writable<IvarList<4>> = Writable::new(IvarList {
        entsize: 32,
        count: 4,
        ivars: [
            IvarEntry { offset: &raw mut OFFSET_keyWindow, name: c"_keyWindow".as_ptr(), kind: c"B".as_ptr(), alignment_raw: 0, size: 1 },
            IvarEntry { offset: &raw mut OFFSET_rootViewController, name: c"_rootViewController".as_ptr(), kind: c"@\"UIViewController\"".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_screen, name: c"_screen".as_ptr(), kind: c"@\"UIScreen\"".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_windowLevel, name: c"_windowLevel".as_ptr(), kind: c"d".as_ptr(), alignment_raw: 3, size: 8 },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static PROPERTIES: Writable<PropertyList<4>> = Writable::new(PropertyList {
        entsize: 16,
        count: 4,
        properties: [
            PropertyEntry { name: c"keyWindow".as_ptr(), attributes: c"TB,R,N,GisKeyWindow,V_keyWindow".as_ptr() },
            PropertyEntry { name: c"rootViewController".as_ptr(), attributes: c"T@\"UIViewController\",&,N,V_rootViewController".as_ptr() },
            PropertyEntry { name: c"screen".as_ptr(), attributes: c"T@\"UIScreen\",&,N,V_screen".as_ptr() },
            PropertyEntry { name: c"windowLevel".as_ptr(), attributes: c"Td,N,V_windowLevel".as_ptr() },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x184,
        instance_start: 224,
        instance_size: 256,
        reserved: 0,
        ivar_layout: b"\x12\0".as_ptr(),
        name: c"UIWindow".as_ptr(),
        methods: METHODS.ptr(),
        protocols: std::ptr::null(),
        ivars: IVARS.ptr(),
        weak_ivar_layout: std::ptr::null(),
        properties: PROPERTIES.ptr(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x185,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIWindow".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIWindow"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: super::UIView::META.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIWindow"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: super::UIView::CLASS.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIScrollView : UIView ------------------------------------------
pub mod UIScrollView {
    use super::*;

    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_scrollEnabled: i32 = 224;
    pub struct I_scrollEnabled;
    impl IvarOffset for I_scrollEnabled {
        fn offset() -> *const i32 {
            &raw const OFFSET_scrollEnabled
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_delegate: i32 = 232;
    pub struct I_delegate;
    impl IvarOffset for I_delegate {
        fn offset() -> *const i32 {
            &raw const OFFSET_delegate
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_contentOffset: i32 = 240;
    pub struct I_contentOffset;
    impl IvarOffset for I_contentOffset {
        fn offset() -> *const i32 {
            &raw const OFFSET_contentOffset
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_contentSize: i32 = 256;
    pub struct I_contentSize;
    impl IvarOffset for I_contentSize {
        fn offset() -> *const i32 {
            &raw const OFFSET_contentSize
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_contentInset: i32 = 272;
    pub struct I_contentInset;
    impl IvarOffset for I_contentInset {
        fn offset() -> *const i32 {
            &raw const OFFSET_contentInset
        }
    }
    unsafe extern "C" fn cxx_destruct(this: Id, _: Sel) {
        destruct(this, &[], &[&raw const OFFSET_delegate]);
    }

    #[link_section = "__DATA,__objc_const"]
    static METHODS: Writable<MethodList<11>> = Writable::new(MethodList {
        entsize_flags: 24,
        count: 11,
        methods: [
            MethodEntry { name: c"contentOffset".as_ptr(), types: c"{CGPoint=dd}16@0:8".as_ptr(), imp: (get_value::<CGPoint, I_contentOffset> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setContentOffset:".as_ptr(), types: c"v32@0:8{CGPoint=dd}16".as_ptr(), imp: (set_value::<CGPoint, I_contentOffset> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c"contentSize".as_ptr(), types: c"{CGSize=dd}16@0:8".as_ptr(), imp: (get_value::<CGSize, I_contentSize> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setContentSize:".as_ptr(), types: c"v32@0:8{CGSize=dd}16".as_ptr(), imp: (set_value::<CGSize, I_contentSize> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c"contentInset".as_ptr(), types: c"{UIEdgeInsets=dddd}16@0:8".as_ptr(), imp: (get_value::<UIEdgeInsets, I_contentInset> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setContentInset:".as_ptr(), types: c"v48@0:8{UIEdgeInsets=dddd}16".as_ptr(), imp: (set_value::<UIEdgeInsets, I_contentInset> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c"isScrollEnabled".as_ptr(), types: c"B16@0:8".as_ptr(), imp: (get_value::<bool, I_scrollEnabled> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setScrollEnabled:".as_ptr(), types: c"v20@0:8B16".as_ptr(), imp: (set_value::<bool, I_scrollEnabled> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c"delegate".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_weak::<I_delegate> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setDelegate:".as_ptr(), types: c"v24@0:8@16".as_ptr(), imp: (set_weak::<I_delegate> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c".cxx_destruct".as_ptr(), types: c"v16@0:8".as_ptr(), imp: (cxx_destruct) as *const c_void },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static IVARS: Writable<IvarList<5>> = Writable::new(IvarList {
        entsize: 32,
        count: 5,
        ivars: [
            IvarEntry { offset: &raw mut OFFSET_scrollEnabled, name: c"_scrollEnabled".as_ptr(), kind: c"B".as_ptr(), alignment_raw: 0, size: 1 },
            IvarEntry { offset: &raw mut OFFSET_delegate, name: c"_delegate".as_ptr(), kind: c"@".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_contentOffset, name: c"_contentOffset".as_ptr(), kind: c"{CGPoint=\"x\"d\"y\"d}".as_ptr(), alignment_raw: 3, size: 16 },
            IvarEntry { offset: &raw mut OFFSET_contentSize, name: c"_contentSize".as_ptr(), kind: c"{CGSize=\"width\"d\"height\"d}".as_ptr(), alignment_raw: 3, size: 16 },
            IvarEntry { offset: &raw mut OFFSET_contentInset, name: c"_contentInset".as_ptr(), kind: c"{UIEdgeInsets=\"top\"d\"left\"d\"bottom\"d\"right\"d}".as_ptr(), alignment_raw: 3, size: 32 },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static PROPERTIES: Writable<PropertyList<5>> = Writable::new(PropertyList {
        entsize: 16,
        count: 5,
        properties: [
            PropertyEntry { name: c"contentInset".as_ptr(), attributes: c"T{UIEdgeInsets=dddd},N,V_contentInset".as_ptr() },
            PropertyEntry { name: c"contentOffset".as_ptr(), attributes: c"T{CGPoint=dd},N,V_contentOffset".as_ptr() },
            PropertyEntry { name: c"contentSize".as_ptr(), attributes: c"T{CGSize=dd},N,V_contentSize".as_ptr() },
            PropertyEntry { name: c"delegate".as_ptr(), attributes: c"T@,W,N,V_delegate".as_ptr() },
            PropertyEntry { name: c"scrollEnabled".as_ptr(), attributes: c"TB,N,GisScrollEnabled,V_scrollEnabled".as_ptr() },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x184,
        instance_start: 224,
        instance_size: 304,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIScrollView".as_ptr(),
        methods: METHODS.ptr(),
        protocols: std::ptr::null(),
        ivars: IVARS.ptr(),
        weak_ivar_layout: b"\x11\0".as_ptr(),
        properties: PROPERTIES.ptr(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x185,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIScrollView".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIScrollView"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: super::UIView::META.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIScrollView"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: super::UIView::CLASS.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UITextView : UIScrollView --------------------------------------
pub mod UITextView {
    use super::*;

    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_editable: i32 = 304;
    pub struct I_editable;
    impl IvarOffset for I_editable {
        fn offset() -> *const i32 {
            &raw const OFFSET_editable
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_text: i32 = 312;
    pub struct I_text;
    impl IvarOffset for I_text {
        fn offset() -> *const i32 {
            &raw const OFFSET_text
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_selectedRange: i32 = 320;
    pub struct I_selectedRange;
    impl IvarOffset for I_selectedRange {
        fn offset() -> *const i32 {
            &raw const OFFSET_selectedRange
        }
    }
    unsafe extern "C" fn cxx_destruct(this: Id, _: Sel) {
        destruct(this, &[&raw const OFFSET_text], &[]);
    }

    #[link_section = "__DATA,__objc_const"]
    static METHODS: Writable<MethodList<7>> = Writable::new(MethodList {
        entsize_flags: 24,
        count: 7,
        methods: [
            MethodEntry { name: c"text".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_value::<Id, I_text> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setText:".as_ptr(), types: c"v24@0:8@16".as_ptr(), imp: (set_copy::<I_text> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c"selectedRange".as_ptr(), types: c"{_NSRange=QQ}16@0:8".as_ptr(), imp: (get_value::<NSRange, I_selectedRange> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setSelectedRange:".as_ptr(), types: c"v32@0:8{_NSRange=QQ}16".as_ptr(), imp: (set_value::<NSRange, I_selectedRange> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c"isEditable".as_ptr(), types: c"B16@0:8".as_ptr(), imp: (get_value::<bool, I_editable> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setEditable:".as_ptr(), types: c"v20@0:8B16".as_ptr(), imp: (set_value::<bool, I_editable> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c".cxx_destruct".as_ptr(), types: c"v16@0:8".as_ptr(), imp: (cxx_destruct) as *const c_void },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static IVARS: Writable<IvarList<3>> = Writable::new(IvarList {
        entsize: 32,
        count: 3,
        ivars: [
            IvarEntry { offset: &raw mut OFFSET_editable, name: c"_editable".as_ptr(), kind: c"B".as_ptr(), alignment_raw: 0, size: 1 },
            IvarEntry { offset: &raw mut OFFSET_text, name: c"_text".as_ptr(), kind: c"@\"NSString\"".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_selectedRange, name: c"_selectedRange".as_ptr(), kind: c"{_NSRange=\"location\"Q\"length\"Q}".as_ptr(), alignment_raw: 3, size: 16 },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static PROPERTIES: Writable<PropertyList<3>> = Writable::new(PropertyList {
        entsize: 16,
        count: 3,
        properties: [
            PropertyEntry { name: c"editable".as_ptr(), attributes: c"TB,N,GisEditable,V_editable".as_ptr() },
            PropertyEntry { name: c"selectedRange".as_ptr(), attributes: c"T{_NSRange=QQ},N,V_selectedRange".as_ptr() },
            PropertyEntry { name: c"text".as_ptr(), attributes: c"T@\"NSString\",C,N,V_text".as_ptr() },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x184,
        instance_start: 304,
        instance_size: 336,
        reserved: 0,
        ivar_layout: b"\x11\0".as_ptr(),
        name: c"UITextView".as_ptr(),
        methods: METHODS.ptr(),
        protocols: std::ptr::null(),
        ivars: IVARS.ptr(),
        weak_ivar_layout: std::ptr::null(),
        properties: PROPERTIES.ptr(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x185,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UITextView".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UITextView"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: super::UIScrollView::META.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UITextView"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: super::UIScrollView::CLASS.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIAlertAction : NSObject ---------------------------------------
pub mod UIAlertAction {
    use super::*;

    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_enabled: i32 = 8;
    pub struct I_enabled;
    impl IvarOffset for I_enabled {
        fn offset() -> *const i32 {
            &raw const OFFSET_enabled
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_title: i32 = 16;
    pub struct I_title;
    impl IvarOffset for I_title {
        fn offset() -> *const i32 {
            &raw const OFFSET_title
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_style: i32 = 24;
    pub struct I_style;
    impl IvarOffset for I_style {
        fn offset() -> *const i32 {
            &raw const OFFSET_style
        }
    }
    unsafe extern "C" fn cxx_destruct(this: Id, _: Sel) {
        destruct(this, &[&raw const OFFSET_title], &[]);
    }

    #[link_section = "__DATA,__objc_const"]
    static METHODS: Writable<MethodList<5>> = Writable::new(MethodList {
        entsize_flags: 24,
        count: 5,
        methods: [
            MethodEntry { name: c"title".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_value::<Id, I_title> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"style".as_ptr(), types: c"q16@0:8".as_ptr(), imp: (get_value::<isize, I_style> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"isEnabled".as_ptr(), types: c"B16@0:8".as_ptr(), imp: (get_value::<bool, I_enabled> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setEnabled:".as_ptr(), types: c"v20@0:8B16".as_ptr(), imp: (set_value::<bool, I_enabled> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c".cxx_destruct".as_ptr(), types: c"v16@0:8".as_ptr(), imp: (cxx_destruct) as *const c_void },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static IVARS: Writable<IvarList<3>> = Writable::new(IvarList {
        entsize: 32,
        count: 3,
        ivars: [
            IvarEntry { offset: &raw mut OFFSET_enabled, name: c"_enabled".as_ptr(), kind: c"B".as_ptr(), alignment_raw: 0, size: 1 },
            IvarEntry { offset: &raw mut OFFSET_title, name: c"_title".as_ptr(), kind: c"@\"NSString\"".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_style, name: c"_style".as_ptr(), kind: c"q".as_ptr(), alignment_raw: 3, size: 8 },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static PROPERTIES: Writable<PropertyList<3>> = Writable::new(PropertyList {
        entsize: 16,
        count: 3,
        properties: [
            PropertyEntry { name: c"enabled".as_ptr(), attributes: c"TB,N,GisEnabled,V_enabled".as_ptr() },
            PropertyEntry { name: c"style".as_ptr(), attributes: c"Tq,R,N,V_style".as_ptr() },
            PropertyEntry { name: c"title".as_ptr(), attributes: c"T@\"NSString\",R,N,V_title".as_ptr() },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x184,
        instance_start: 8,
        instance_size: 32,
        reserved: 0,
        ivar_layout: b"\x11\0".as_ptr(),
        name: c"UIAlertAction".as_ptr(),
        methods: METHODS.ptr(),
        protocols: std::ptr::null(),
        ivars: IVARS.ptr(),
        weak_ivar_layout: std::ptr::null(),
        properties: PROPERTIES.ptr(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x185,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIAlertAction".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIAlertAction"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: &raw const NSObject_META,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIAlertAction"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: &raw const NSObject_CLASS,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIAlertController : UIViewController ---------------------------
pub mod UIAlertController {
    use super::*;

    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_message: i32 = 96;
    pub struct I_message;
    impl IvarOffset for I_message {
        fn offset() -> *const i32 {
            &raw const OFFSET_message
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_preferredStyle: i32 = 104;
    pub struct I_preferredStyle;
    impl IvarOffset for I_preferredStyle {
        fn offset() -> *const i32 {
            &raw const OFFSET_preferredStyle
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_actions: i32 = 112;
    pub struct I_actions;
    impl IvarOffset for I_actions {
        fn offset() -> *const i32 {
            &raw const OFFSET_actions
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_preferredAction: i32 = 120;
    pub struct I_preferredAction;
    impl IvarOffset for I_preferredAction {
        fn offset() -> *const i32 {
            &raw const OFFSET_preferredAction
        }
    }
    unsafe extern "C" fn cxx_destruct(this: Id, _: Sel) {
        destruct(this, &[&raw const OFFSET_message, &raw const OFFSET_actions, &raw const OFFSET_preferredAction], &[]);
    }

    #[link_section = "__DATA,__objc_const"]
    static METHODS: Writable<MethodList<7>> = Writable::new(MethodList {
        entsize_flags: 24,
        count: 7,
        methods: [
            MethodEntry { name: c"message".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_value::<Id, I_message> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setMessage:".as_ptr(), types: c"v24@0:8@16".as_ptr(), imp: (set_copy::<I_message> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c"preferredStyle".as_ptr(), types: c"q16@0:8".as_ptr(), imp: (get_value::<isize, I_preferredStyle> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"actions".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_value::<Id, I_actions> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"preferredAction".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_value::<Id, I_preferredAction> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setPreferredAction:".as_ptr(), types: c"v24@0:8@16".as_ptr(), imp: (set_strong::<I_preferredAction> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c".cxx_destruct".as_ptr(), types: c"v16@0:8".as_ptr(), imp: (cxx_destruct) as *const c_void },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static IVARS: Writable<IvarList<4>> = Writable::new(IvarList {
        entsize: 32,
        count: 4,
        ivars: [
            IvarEntry { offset: &raw mut OFFSET_message, name: c"_message".as_ptr(), kind: c"@\"NSString\"".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_preferredStyle, name: c"_preferredStyle".as_ptr(), kind: c"q".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_actions, name: c"_actions".as_ptr(), kind: c"@\"NSArray\"".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_preferredAction, name: c"_preferredAction".as_ptr(), kind: c"@\"UIAlertAction\"".as_ptr(), alignment_raw: 3, size: 8 },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static PROPERTIES: Writable<PropertyList<4>> = Writable::new(PropertyList {
        entsize: 16,
        count: 4,
        properties: [
            PropertyEntry { name: c"actions".as_ptr(), attributes: c"T@\"NSArray\",R,N,V_actions".as_ptr() },
            PropertyEntry { name: c"message".as_ptr(), attributes: c"T@\"NSString\",C,N,V_message".as_ptr() },
            PropertyEntry { name: c"preferredAction".as_ptr(), attributes: c"T@\"UIAlertAction\",&,N,V_preferredAction".as_ptr() },
            PropertyEntry { name: c"preferredStyle".as_ptr(), attributes: c"Tq,R,N,V_preferredStyle".as_ptr() },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x184,
        instance_start: 96,
        instance_size: 128,
        reserved: 0,
        ivar_layout: b"\x01\x12\0".as_ptr(),
        name: c"UIAlertController".as_ptr(),
        methods: METHODS.ptr(),
        protocols: std::ptr::null(),
        ivars: IVARS.ptr(),
        weak_ivar_layout: std::ptr::null(),
        properties: PROPERTIES.ptr(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x185,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIAlertController".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIAlertController"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: super::UIViewController::META.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIAlertController"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: super::UIViewController::CLASS.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UINib : NSObject -----------------------------------------------
pub mod UINib {
    use super::*;


    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 8,
        instance_size: 8,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UINib".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UINib".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UINib"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: &raw const NSObject_META,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UINib"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: &raw const NSObject_CLASS,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIApplication : UIResponder ------------------------------------
pub mod UIApplication {
    use super::*;

    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_idleTimerDisabled: i32 = 24;
    pub struct I_idleTimerDisabled;
    impl IvarOffset for I_idleTimerDisabled {
        fn offset() -> *const i32 {
            &raw const OFFSET_idleTimerDisabled
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_isProtectedDataAvailable: i32 = 25;
    pub struct I_isProtectedDataAvailable;
    impl IvarOffset for I_isProtectedDataAvailable {
        fn offset() -> *const i32 {
            &raw const OFFSET_isProtectedDataAvailable
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_delegate: i32 = 32;
    pub struct I_delegate;
    impl IvarOffset for I_delegate {
        fn offset() -> *const i32 {
            &raw const OFFSET_delegate
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_keyWindow: i32 = 40;
    pub struct I_keyWindow;
    impl IvarOffset for I_keyWindow {
        fn offset() -> *const i32 {
            &raw const OFFSET_keyWindow
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_windows: i32 = 48;
    pub struct I_windows;
    impl IvarOffset for I_windows {
        fn offset() -> *const i32 {
            &raw const OFFSET_windows
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_applicationState: i32 = 56;
    pub struct I_applicationState;
    impl IvarOffset for I_applicationState {
        fn offset() -> *const i32 {
            &raw const OFFSET_applicationState
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_statusBarOrientation: i32 = 64;
    pub struct I_statusBarOrientation;
    impl IvarOffset for I_statusBarOrientation {
        fn offset() -> *const i32 {
            &raw const OFFSET_statusBarOrientation
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_applicationIconBadgeNumber: i32 = 72;
    pub struct I_applicationIconBadgeNumber;
    impl IvarOffset for I_applicationIconBadgeNumber {
        fn offset() -> *const i32 {
            &raw const OFFSET_applicationIconBadgeNumber
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_backgroundTimeRemaining: i32 = 80;
    pub struct I_backgroundTimeRemaining;
    impl IvarOffset for I_backgroundTimeRemaining {
        fn offset() -> *const i32 {
            &raw const OFFSET_backgroundTimeRemaining
        }
    }
    unsafe extern "C" fn cxx_destruct(this: Id, _: Sel) {
        destruct(this, &[&raw const OFFSET_keyWindow, &raw const OFFSET_windows], &[&raw const OFFSET_delegate]);
    }

    #[link_section = "__DATA,__objc_const"]
    static METHODS: Writable<MethodList<13>> = Writable::new(MethodList {
        entsize_flags: 24,
        count: 13,
        methods: [
            MethodEntry { name: c"delegate".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_weak::<I_delegate> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setDelegate:".as_ptr(), types: c"v24@0:8@16".as_ptr(), imp: (set_weak::<I_delegate> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c"keyWindow".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_value::<Id, I_keyWindow> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"windows".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_value::<Id, I_windows> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"applicationState".as_ptr(), types: c"q16@0:8".as_ptr(), imp: (get_value::<isize, I_applicationState> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"isIdleTimerDisabled".as_ptr(), types: c"B16@0:8".as_ptr(), imp: (get_value::<bool, I_idleTimerDisabled> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setIdleTimerDisabled:".as_ptr(), types: c"v20@0:8B16".as_ptr(), imp: (set_value::<bool, I_idleTimerDisabled> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c"statusBarOrientation".as_ptr(), types: c"q16@0:8".as_ptr(), imp: (get_value::<isize, I_statusBarOrientation> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"applicationIconBadgeNumber".as_ptr(), types: c"q16@0:8".as_ptr(), imp: (get_value::<isize, I_applicationIconBadgeNumber> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"setApplicationIconBadgeNumber:".as_ptr(), types: c"v24@0:8q16".as_ptr(), imp: (set_value::<isize, I_applicationIconBadgeNumber> as unsafe extern "C" fn(_, _, _) -> _) as *const c_void },
            MethodEntry { name: c"backgroundTimeRemaining".as_ptr(), types: c"d16@0:8".as_ptr(), imp: (get_value::<f64, I_backgroundTimeRemaining> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"isProtectedDataAvailable".as_ptr(), types: c"B16@0:8".as_ptr(), imp: (get_value::<bool, I_isProtectedDataAvailable> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c".cxx_destruct".as_ptr(), types: c"v16@0:8".as_ptr(), imp: (cxx_destruct) as *const c_void },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static IVARS: Writable<IvarList<9>> = Writable::new(IvarList {
        entsize: 32,
        count: 9,
        ivars: [
            IvarEntry { offset: &raw mut OFFSET_idleTimerDisabled, name: c"_idleTimerDisabled".as_ptr(), kind: c"B".as_ptr(), alignment_raw: 0, size: 1 },
            IvarEntry { offset: &raw mut OFFSET_isProtectedDataAvailable, name: c"_isProtectedDataAvailable".as_ptr(), kind: c"B".as_ptr(), alignment_raw: 0, size: 1 },
            IvarEntry { offset: &raw mut OFFSET_delegate, name: c"_delegate".as_ptr(), kind: c"@".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_keyWindow, name: c"_keyWindow".as_ptr(), kind: c"@\"UIWindow\"".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_windows, name: c"_windows".as_ptr(), kind: c"@\"NSArray\"".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_applicationState, name: c"_applicationState".as_ptr(), kind: c"q".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_statusBarOrientation, name: c"_statusBarOrientation".as_ptr(), kind: c"q".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_applicationIconBadgeNumber, name: c"_applicationIconBadgeNumber".as_ptr(), kind: c"q".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_backgroundTimeRemaining, name: c"_backgroundTimeRemaining".as_ptr(), kind: c"d".as_ptr(), alignment_raw: 3, size: 8 },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static PROPERTIES: Writable<PropertyList<9>> = Writable::new(PropertyList {
        entsize: 16,
        count: 9,
        properties: [
            PropertyEntry { name: c"applicationIconBadgeNumber".as_ptr(), attributes: c"Tq,N,V_applicationIconBadgeNumber".as_ptr() },
            PropertyEntry { name: c"applicationState".as_ptr(), attributes: c"Tq,R,N,V_applicationState".as_ptr() },
            PropertyEntry { name: c"backgroundTimeRemaining".as_ptr(), attributes: c"Td,R,N,V_backgroundTimeRemaining".as_ptr() },
            PropertyEntry { name: c"delegate".as_ptr(), attributes: c"T@,W,N,V_delegate".as_ptr() },
            PropertyEntry { name: c"idleTimerDisabled".as_ptr(), attributes: c"TB,N,GisIdleTimerDisabled,V_idleTimerDisabled".as_ptr() },
            PropertyEntry { name: c"isProtectedDataAvailable".as_ptr(), attributes: c"TB,R,N,V_isProtectedDataAvailable".as_ptr() },
            PropertyEntry { name: c"keyWindow".as_ptr(), attributes: c"T@\"UIWindow\",R,N,V_keyWindow".as_ptr() },
            PropertyEntry { name: c"statusBarOrientation".as_ptr(), attributes: c"Tq,R,N,V_statusBarOrientation".as_ptr() },
            PropertyEntry { name: c"windows".as_ptr(), attributes: c"T@\"NSArray\",R,N,V_windows".as_ptr() },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x184,
        instance_start: 24,
        instance_size: 88,
        reserved: 0,
        ivar_layout: b"\x22\0".as_ptr(),
        name: c"UIApplication".as_ptr(),
        methods: METHODS.ptr(),
        protocols: std::ptr::null(),
        ivars: IVARS.ptr(),
        weak_ivar_layout: b"\x11\0".as_ptr(),
        properties: PROPERTIES.ptr(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x185,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIApplication".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIApplication"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: super::UIResponder::META.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIApplication"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: super::UIResponder::CLASS.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UITouch : NSObject ---------------------------------------------
pub mod UITouch {
    use super::*;

    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_timestamp: i32 = 8;
    pub struct I_timestamp;
    impl IvarOffset for I_timestamp {
        fn offset() -> *const i32 {
            &raw const OFFSET_timestamp
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_phase: i32 = 16;
    pub struct I_phase;
    impl IvarOffset for I_phase {
        fn offset() -> *const i32 {
            &raw const OFFSET_phase
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_tapCount: i32 = 24;
    pub struct I_tapCount;
    impl IvarOffset for I_tapCount {
        fn offset() -> *const i32 {
            &raw const OFFSET_tapCount
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_type: i32 = 32;
    pub struct I_type;
    impl IvarOffset for I_type {
        fn offset() -> *const i32 {
            &raw const OFFSET_type
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_force: i32 = 40;
    pub struct I_force;
    impl IvarOffset for I_force {
        fn offset() -> *const i32 {
            &raw const OFFSET_force
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_maximumPossibleForce: i32 = 48;
    pub struct I_maximumPossibleForce;
    impl IvarOffset for I_maximumPossibleForce {
        fn offset() -> *const i32 {
            &raw const OFFSET_maximumPossibleForce
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_view: i32 = 56;
    pub struct I_view;
    impl IvarOffset for I_view {
        fn offset() -> *const i32 {
            &raw const OFFSET_view
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_window: i32 = 64;
    pub struct I_window;
    impl IvarOffset for I_window {
        fn offset() -> *const i32 {
            &raw const OFFSET_window
        }
    }
    unsafe extern "C" fn cxx_destruct(this: Id, _: Sel) {
        destruct(this, &[&raw const OFFSET_view, &raw const OFFSET_window], &[]);
    }

    #[link_section = "__DATA,__objc_const"]
    static METHODS: Writable<MethodList<9>> = Writable::new(MethodList {
        entsize_flags: 24,
        count: 9,
        methods: [
            MethodEntry { name: c"timestamp".as_ptr(), types: c"d16@0:8".as_ptr(), imp: (get_value::<f64, I_timestamp> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"phase".as_ptr(), types: c"q16@0:8".as_ptr(), imp: (get_value::<isize, I_phase> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"tapCount".as_ptr(), types: c"Q16@0:8".as_ptr(), imp: (get_value::<usize, I_tapCount> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"type".as_ptr(), types: c"q16@0:8".as_ptr(), imp: (get_value::<isize, I_type> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"force".as_ptr(), types: c"d16@0:8".as_ptr(), imp: (get_value::<f64, I_force> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"maximumPossibleForce".as_ptr(), types: c"d16@0:8".as_ptr(), imp: (get_value::<f64, I_maximumPossibleForce> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"view".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_value::<Id, I_view> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"window".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_value::<Id, I_window> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c".cxx_destruct".as_ptr(), types: c"v16@0:8".as_ptr(), imp: (cxx_destruct) as *const c_void },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static IVARS: Writable<IvarList<8>> = Writable::new(IvarList {
        entsize: 32,
        count: 8,
        ivars: [
            IvarEntry { offset: &raw mut OFFSET_timestamp, name: c"_timestamp".as_ptr(), kind: c"d".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_phase, name: c"_phase".as_ptr(), kind: c"q".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_tapCount, name: c"_tapCount".as_ptr(), kind: c"Q".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_type, name: c"_type".as_ptr(), kind: c"q".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_force, name: c"_force".as_ptr(), kind: c"d".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_maximumPossibleForce, name: c"_maximumPossibleForce".as_ptr(), kind: c"d".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_view, name: c"_view".as_ptr(), kind: c"@\"UIView\"".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_window, name: c"_window".as_ptr(), kind: c"@\"UIWindow\"".as_ptr(), alignment_raw: 3, size: 8 },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static PROPERTIES: Writable<PropertyList<8>> = Writable::new(PropertyList {
        entsize: 16,
        count: 8,
        properties: [
            PropertyEntry { name: c"force".as_ptr(), attributes: c"Td,R,N,V_force".as_ptr() },
            PropertyEntry { name: c"maximumPossibleForce".as_ptr(), attributes: c"Td,R,N,V_maximumPossibleForce".as_ptr() },
            PropertyEntry { name: c"phase".as_ptr(), attributes: c"Tq,R,N,V_phase".as_ptr() },
            PropertyEntry { name: c"tapCount".as_ptr(), attributes: c"TQ,R,N,V_tapCount".as_ptr() },
            PropertyEntry { name: c"timestamp".as_ptr(), attributes: c"Td,R,N,V_timestamp".as_ptr() },
            PropertyEntry { name: c"type".as_ptr(), attributes: c"Tq,R,N,V_type".as_ptr() },
            PropertyEntry { name: c"view".as_ptr(), attributes: c"T@\"UIView\",R,N,V_view".as_ptr() },
            PropertyEntry { name: c"window".as_ptr(), attributes: c"T@\"UIWindow\",R,N,V_window".as_ptr() },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x184,
        instance_start: 8,
        instance_size: 72,
        reserved: 0,
        ivar_layout: b"\x62\0".as_ptr(),
        name: c"UITouch".as_ptr(),
        methods: METHODS.ptr(),
        protocols: std::ptr::null(),
        ivars: IVARS.ptr(),
        weak_ivar_layout: std::ptr::null(),
        properties: PROPERTIES.ptr(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x185,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UITouch".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UITouch"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: &raw const NSObject_META,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UITouch"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: &raw const NSObject_CLASS,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIEvent : NSObject ---------------------------------------------
pub mod UIEvent {
    use super::*;

    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_type: i32 = 8;
    pub struct I_type;
    impl IvarOffset for I_type {
        fn offset() -> *const i32 {
            &raw const OFFSET_type
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_timestamp: i32 = 16;
    pub struct I_timestamp;
    impl IvarOffset for I_timestamp {
        fn offset() -> *const i32 {
            &raw const OFFSET_timestamp
        }
    }

    #[link_section = "__DATA,__objc_const"]
    static METHODS: Writable<MethodList<2>> = Writable::new(MethodList {
        entsize_flags: 24,
        count: 2,
        methods: [
            MethodEntry { name: c"type".as_ptr(), types: c"q16@0:8".as_ptr(), imp: (get_value::<isize, I_type> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"timestamp".as_ptr(), types: c"d16@0:8".as_ptr(), imp: (get_value::<f64, I_timestamp> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static IVARS: Writable<IvarList<2>> = Writable::new(IvarList {
        entsize: 32,
        count: 2,
        ivars: [
            IvarEntry { offset: &raw mut OFFSET_type, name: c"_type".as_ptr(), kind: c"q".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_timestamp, name: c"_timestamp".as_ptr(), kind: c"d".as_ptr(), alignment_raw: 3, size: 8 },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static PROPERTIES: Writable<PropertyList<2>> = Writable::new(PropertyList {
        entsize: 16,
        count: 2,
        properties: [
            PropertyEntry { name: c"timestamp".as_ptr(), attributes: c"Td,R,N,V_timestamp".as_ptr() },
            PropertyEntry { name: c"type".as_ptr(), attributes: c"Tq,R,N,V_type".as_ptr() },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 8,
        instance_size: 24,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIEvent".as_ptr(),
        methods: METHODS.ptr(),
        protocols: std::ptr::null(),
        ivars: IVARS.ptr(),
        weak_ivar_layout: std::ptr::null(),
        properties: PROPERTIES.ptr(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIEvent".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIEvent"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: &raw const NSObject_META,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIEvent"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: &raw const NSObject_CLASS,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIKey : NSObject -----------------------------------------------
pub mod UIKey {
    use super::*;

    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_characters: i32 = 8;
    pub struct I_characters;
    impl IvarOffset for I_characters {
        fn offset() -> *const i32 {
            &raw const OFFSET_characters
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_charactersIgnoringModifiers: i32 = 16;
    pub struct I_charactersIgnoringModifiers;
    impl IvarOffset for I_charactersIgnoringModifiers {
        fn offset() -> *const i32 {
            &raw const OFFSET_charactersIgnoringModifiers
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_modifierFlags: i32 = 24;
    pub struct I_modifierFlags;
    impl IvarOffset for I_modifierFlags {
        fn offset() -> *const i32 {
            &raw const OFFSET_modifierFlags
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_keyCode: i32 = 32;
    pub struct I_keyCode;
    impl IvarOffset for I_keyCode {
        fn offset() -> *const i32 {
            &raw const OFFSET_keyCode
        }
    }
    unsafe extern "C" fn cxx_destruct(this: Id, _: Sel) {
        destruct(this, &[&raw const OFFSET_characters, &raw const OFFSET_charactersIgnoringModifiers], &[]);
    }

    #[link_section = "__DATA,__objc_const"]
    static METHODS: Writable<MethodList<5>> = Writable::new(MethodList {
        entsize_flags: 24,
        count: 5,
        methods: [
            MethodEntry { name: c"characters".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_value::<Id, I_characters> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"charactersIgnoringModifiers".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_value::<Id, I_charactersIgnoringModifiers> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"modifierFlags".as_ptr(), types: c"q16@0:8".as_ptr(), imp: (get_value::<isize, I_modifierFlags> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"keyCode".as_ptr(), types: c"q16@0:8".as_ptr(), imp: (get_value::<isize, I_keyCode> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c".cxx_destruct".as_ptr(), types: c"v16@0:8".as_ptr(), imp: (cxx_destruct) as *const c_void },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static IVARS: Writable<IvarList<4>> = Writable::new(IvarList {
        entsize: 32,
        count: 4,
        ivars: [
            IvarEntry { offset: &raw mut OFFSET_characters, name: c"_characters".as_ptr(), kind: c"@\"NSString\"".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_charactersIgnoringModifiers, name: c"_charactersIgnoringModifiers".as_ptr(), kind: c"@\"NSString\"".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_modifierFlags, name: c"_modifierFlags".as_ptr(), kind: c"q".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_keyCode, name: c"_keyCode".as_ptr(), kind: c"q".as_ptr(), alignment_raw: 3, size: 8 },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static PROPERTIES: Writable<PropertyList<4>> = Writable::new(PropertyList {
        entsize: 16,
        count: 4,
        properties: [
            PropertyEntry { name: c"characters".as_ptr(), attributes: c"T@\"NSString\",R,N,V_characters".as_ptr() },
            PropertyEntry { name: c"charactersIgnoringModifiers".as_ptr(), attributes: c"T@\"NSString\",R,N,V_charactersIgnoringModifiers".as_ptr() },
            PropertyEntry { name: c"keyCode".as_ptr(), attributes: c"Tq,R,N,V_keyCode".as_ptr() },
            PropertyEntry { name: c"modifierFlags".as_ptr(), attributes: c"Tq,R,N,V_modifierFlags".as_ptr() },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x184,
        instance_start: 8,
        instance_size: 40,
        reserved: 0,
        ivar_layout: b"\x02\0".as_ptr(),
        name: c"UIKey".as_ptr(),
        methods: METHODS.ptr(),
        protocols: std::ptr::null(),
        ivars: IVARS.ptr(),
        weak_ivar_layout: std::ptr::null(),
        properties: PROPERTIES.ptr(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x185,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIKey".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIKey"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: &raw const NSObject_META,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIKey"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: &raw const NSObject_CLASS,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIPress : NSObject ---------------------------------------------
pub mod UIPress {
    use super::*;

    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_timestamp: i32 = 8;
    pub struct I_timestamp;
    impl IvarOffset for I_timestamp {
        fn offset() -> *const i32 {
            &raw const OFFSET_timestamp
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_phase: i32 = 16;
    pub struct I_phase;
    impl IvarOffset for I_phase {
        fn offset() -> *const i32 {
            &raw const OFFSET_phase
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_type: i32 = 24;
    pub struct I_type;
    impl IvarOffset for I_type {
        fn offset() -> *const i32 {
            &raw const OFFSET_type
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_key: i32 = 32;
    pub struct I_key;
    impl IvarOffset for I_key {
        fn offset() -> *const i32 {
            &raw const OFFSET_key
        }
    }
    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_force: i32 = 40;
    pub struct I_force;
    impl IvarOffset for I_force {
        fn offset() -> *const i32 {
            &raw const OFFSET_force
        }
    }
    unsafe extern "C" fn cxx_destruct(this: Id, _: Sel) {
        destruct(this, &[&raw const OFFSET_key], &[]);
    }

    #[link_section = "__DATA,__objc_const"]
    static METHODS: Writable<MethodList<6>> = Writable::new(MethodList {
        entsize_flags: 24,
        count: 6,
        methods: [
            MethodEntry { name: c"timestamp".as_ptr(), types: c"d16@0:8".as_ptr(), imp: (get_value::<f64, I_timestamp> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"phase".as_ptr(), types: c"q16@0:8".as_ptr(), imp: (get_value::<isize, I_phase> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"type".as_ptr(), types: c"q16@0:8".as_ptr(), imp: (get_value::<isize, I_type> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"key".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_value::<Id, I_key> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c"force".as_ptr(), types: c"d16@0:8".as_ptr(), imp: (get_value::<f64, I_force> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c".cxx_destruct".as_ptr(), types: c"v16@0:8".as_ptr(), imp: (cxx_destruct) as *const c_void },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static IVARS: Writable<IvarList<5>> = Writable::new(IvarList {
        entsize: 32,
        count: 5,
        ivars: [
            IvarEntry { offset: &raw mut OFFSET_timestamp, name: c"_timestamp".as_ptr(), kind: c"d".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_phase, name: c"_phase".as_ptr(), kind: c"q".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_type, name: c"_type".as_ptr(), kind: c"q".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_key, name: c"_key".as_ptr(), kind: c"@\"UIKey\"".as_ptr(), alignment_raw: 3, size: 8 },
            IvarEntry { offset: &raw mut OFFSET_force, name: c"_force".as_ptr(), kind: c"d".as_ptr(), alignment_raw: 3, size: 8 },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static PROPERTIES: Writable<PropertyList<5>> = Writable::new(PropertyList {
        entsize: 16,
        count: 5,
        properties: [
            PropertyEntry { name: c"force".as_ptr(), attributes: c"Td,R,N,V_force".as_ptr() },
            PropertyEntry { name: c"key".as_ptr(), attributes: c"T@\"UIKey\",R,N,V_key".as_ptr() },
            PropertyEntry { name: c"phase".as_ptr(), attributes: c"Tq,R,N,V_phase".as_ptr() },
            PropertyEntry { name: c"timestamp".as_ptr(), attributes: c"Td,R,N,V_timestamp".as_ptr() },
            PropertyEntry { name: c"type".as_ptr(), attributes: c"Tq,R,N,V_type".as_ptr() },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x184,
        instance_start: 8,
        instance_size: 48,
        reserved: 0,
        ivar_layout: b"\x31\0".as_ptr(),
        name: c"UIPress".as_ptr(),
        methods: METHODS.ptr(),
        protocols: std::ptr::null(),
        ivars: IVARS.ptr(),
        weak_ivar_layout: std::ptr::null(),
        properties: PROPERTIES.ptr(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x185,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIPress".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIPress"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: &raw const NSObject_META,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIPress"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: &raw const NSObject_CLASS,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIPressesEvent : UIEvent ---------------------------------------
pub mod UIPressesEvent {
    use super::*;

    #[link_section = "__DATA,__objc_ivar"]
    pub static mut OFFSET_allPresses: i32 = 24;
    pub struct I_allPresses;
    impl IvarOffset for I_allPresses {
        fn offset() -> *const i32 {
            &raw const OFFSET_allPresses
        }
    }
    unsafe extern "C" fn cxx_destruct(this: Id, _: Sel) {
        destruct(this, &[&raw const OFFSET_allPresses], &[]);
    }

    #[link_section = "__DATA,__objc_const"]
    static METHODS: Writable<MethodList<2>> = Writable::new(MethodList {
        entsize_flags: 24,
        count: 2,
        methods: [
            MethodEntry { name: c"allPresses".as_ptr(), types: c"@16@0:8".as_ptr(), imp: (get_value::<Id, I_allPresses> as unsafe extern "C" fn(_, _) -> _) as *const c_void },
            MethodEntry { name: c".cxx_destruct".as_ptr(), types: c"v16@0:8".as_ptr(), imp: (cxx_destruct) as *const c_void },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static IVARS: Writable<IvarList<1>> = Writable::new(IvarList {
        entsize: 32,
        count: 1,
        ivars: [
            IvarEntry { offset: &raw mut OFFSET_allPresses, name: c"_allPresses".as_ptr(), kind: c"@\"NSSet\"".as_ptr(), alignment_raw: 3, size: 8 },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static PROPERTIES: Writable<PropertyList<1>> = Writable::new(PropertyList {
        entsize: 16,
        count: 1,
        properties: [
            PropertyEntry { name: c"allPresses".as_ptr(), attributes: c"T@\"NSSet\",R,N,V_allPresses".as_ptr() },
        ],
    });
    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x184,
        instance_start: 24,
        instance_size: 32,
        reserved: 0,
        ivar_layout: b"\x01\0".as_ptr(),
        name: c"UIPressesEvent".as_ptr(),
        methods: METHODS.ptr(),
        protocols: std::ptr::null(),
        ivars: IVARS.ptr(),
        weak_ivar_layout: std::ptr::null(),
        properties: PROPERTIES.ptr(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x185,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIPressesEvent".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIPressesEvent"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: super::UIEvent::META.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIPressesEvent"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: super::UIEvent::CLASS.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UINavigationController : UIViewController ----------------------
pub mod UINavigationController {
    use super::*;


    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 96,
        instance_size: 96,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UINavigationController".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UINavigationController".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UINavigationController"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: super::UIViewController::META.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UINavigationController"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: super::UIViewController::CLASS.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIImage : NSObject ---------------------------------------------
pub mod UIImage {
    use super::*;


    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 8,
        instance_size: 8,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIImage".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIImage".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIImage"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: &raw const NSObject_META,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIImage"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: &raw const NSObject_CLASS,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIFont : NSObject ----------------------------------------------
pub mod UIFont {
    use super::*;


    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 8,
        instance_size: 8,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIFont".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIFont".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIFont"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: &raw const NSObject_META,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIFont"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: &raw const NSObject_CLASS,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UILabel : UIView -----------------------------------------------
pub mod UILabel {
    use super::*;


    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 224,
        instance_size: 224,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UILabel".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UILabel".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UILabel"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: super::UIView::META.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UILabel"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: super::UIView::CLASS.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIProgressView : UIView ----------------------------------------
pub mod UIProgressView {
    use super::*;


    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 224,
        instance_size: 224,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIProgressView".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIProgressView".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIProgressView"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: super::UIView::META.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIProgressView"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: super::UIView::CLASS.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIPasteboard : NSObject ----------------------------------------
pub mod UIPasteboard {
    use super::*;


    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 8,
        instance_size: 8,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIPasteboard".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIPasteboard".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIPasteboard"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: &raw const NSObject_META,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIPasteboard"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: &raw const NSObject_CLASS,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIFeedbackGenerator : NSObject ---------------------------------
pub mod UIFeedbackGenerator {
    use super::*;


    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 8,
        instance_size: 8,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIFeedbackGenerator".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIFeedbackGenerator".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIFeedbackGenerator"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: &raw const NSObject_META,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIFeedbackGenerator"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: &raw const NSObject_CLASS,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIImpactFeedbackGenerator : UIFeedbackGenerator ----------------
pub mod UIImpactFeedbackGenerator {
    use super::*;


    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 8,
        instance_size: 8,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIImpactFeedbackGenerator".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIImpactFeedbackGenerator".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIImpactFeedbackGenerator"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: super::UIFeedbackGenerator::META.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIImpactFeedbackGenerator"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: super::UIFeedbackGenerator::CLASS.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UINotificationFeedbackGenerator : UIFeedbackGenerator ----------
pub mod UINotificationFeedbackGenerator {
    use super::*;


    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 8,
        instance_size: 8,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UINotificationFeedbackGenerator".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UINotificationFeedbackGenerator".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UINotificationFeedbackGenerator"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: super::UIFeedbackGenerator::META.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UINotificationFeedbackGenerator"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: super::UIFeedbackGenerator::CLASS.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UISelectionFeedbackGenerator : UIFeedbackGenerator -------------
pub mod UISelectionFeedbackGenerator {
    use super::*;


    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 8,
        instance_size: 8,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UISelectionFeedbackGenerator".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UISelectionFeedbackGenerator".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UISelectionFeedbackGenerator"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: super::UIFeedbackGenerator::META.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UISelectionFeedbackGenerator"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: super::UIFeedbackGenerator::CLASS.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIStoryboard : NSObject ----------------------------------------
pub mod UIStoryboard {
    use super::*;


    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 8,
        instance_size: 8,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIStoryboard".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIStoryboard".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIStoryboard"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: &raw const NSObject_META,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIStoryboard"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: &raw const NSObject_CLASS,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIAccessibilityElement : UIResponder ---------------------------
pub mod UIAccessibilityElement {
    use super::*;


    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 24,
        instance_size: 24,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIAccessibilityElement".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIAccessibilityElement".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIAccessibilityElement"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: super::UIResponder::META.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIAccessibilityElement"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: super::UIResponder::CLASS.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIPointerInteraction : NSObject --------------------------------
pub mod UIPointerInteraction {
    use super::*;


    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 8,
        instance_size: 8,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIPointerInteraction".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIPointerInteraction".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIPointerInteraction"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: &raw const NSObject_META,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIPointerInteraction"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: &raw const NSObject_CLASS,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UITextPosition : NSObject --------------------------------------
pub mod UITextPosition {
    use super::*;


    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 8,
        instance_size: 8,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UITextPosition".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UITextPosition".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UITextPosition"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: &raw const NSObject_META,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UITextPosition"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: &raw const NSObject_CLASS,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UITextRange : NSObject -----------------------------------------
pub mod UITextRange {
    use super::*;


    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 8,
        instance_size: 8,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UITextRange".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UITextRange".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UITextRange"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: &raw const NSObject_META,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UITextRange"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: &raw const NSObject_CLASS,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIUserNotificationAction : NSObject ----------------------------
pub mod UIUserNotificationAction {
    use super::*;


    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 8,
        instance_size: 8,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIUserNotificationAction".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIUserNotificationAction".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIUserNotificationAction"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: &raw const NSObject_META,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIUserNotificationAction"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: &raw const NSObject_CLASS,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIMutableUserNotificationAction : UIUserNotificationAction -----
pub mod UIMutableUserNotificationAction {
    use super::*;


    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 8,
        instance_size: 8,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIMutableUserNotificationAction".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIMutableUserNotificationAction".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIMutableUserNotificationAction"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: super::UIUserNotificationAction::META.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIMutableUserNotificationAction"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: super::UIUserNotificationAction::CLASS.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIUserNotificationCategory : NSObject --------------------------
pub mod UIUserNotificationCategory {
    use super::*;


    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 8,
        instance_size: 8,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIUserNotificationCategory".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIUserNotificationCategory".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIUserNotificationCategory"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: &raw const NSObject_META,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIUserNotificationCategory"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: &raw const NSObject_CLASS,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIMutableUserNotificationCategory : UIUserNotificationCategory -
pub mod UIMutableUserNotificationCategory {
    use super::*;


    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 8,
        instance_size: 8,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIMutableUserNotificationCategory".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIMutableUserNotificationCategory".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIMutableUserNotificationCategory"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: super::UIUserNotificationCategory::META.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIMutableUserNotificationCategory"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: super::UIUserNotificationCategory::CLASS.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIUserNotificationSettings : NSObject --------------------------
pub mod UIUserNotificationSettings {
    use super::*;


    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 8,
        instance_size: 8,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIUserNotificationSettings".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIUserNotificationSettings".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIUserNotificationSettings"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: &raw const NSObject_META,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIUserNotificationSettings"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: &raw const NSObject_CLASS,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIBarItem : NSObject -------------------------------------------
pub mod UIBarItem {
    use super::*;


    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 8,
        instance_size: 8,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIBarItem".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIBarItem".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIBarItem"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: &raw const NSObject_META,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIBarItem"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: &raw const NSObject_CLASS,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIBarButtonItem : UIBarItem ------------------------------------
pub mod UIBarButtonItem {
    use super::*;


    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 8,
        instance_size: 8,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIBarButtonItem".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIBarButtonItem".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIBarButtonItem"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: super::UIBarItem::META.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIBarButtonItem"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: super::UIBarItem::CLASS.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UITabBarController : UIViewController --------------------------
pub mod UITabBarController {
    use super::*;


    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 96,
        instance_size: 96,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UITabBarController".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UITabBarController".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UITabBarController"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: super::UIViewController::META.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UITabBarController"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: super::UIViewController::CLASS.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIActivityViewController : UIViewController --------------------
pub mod UIActivityViewController {
    use super::*;


    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 96,
        instance_size: 96,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIActivityViewController".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIActivityViewController".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIActivityViewController"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: super::UIViewController::META.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIActivityViewController"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: super::UIViewController::CLASS.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIDocumentPickerViewController : UIViewController --------------
pub mod UIDocumentPickerViewController {
    use super::*;


    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 96,
        instance_size: 96,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIDocumentPickerViewController".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIDocumentPickerViewController".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIDocumentPickerViewController"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: super::UIViewController::META.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIDocumentPickerViewController"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: super::UIViewController::CLASS.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIImagePickerController : UINavigationController ---------------
pub mod UIImagePickerController {
    use super::*;


    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 96,
        instance_size: 96,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIImagePickerController".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIImagePickerController".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIImagePickerController"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: super::UINavigationController::META.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIImagePickerController"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: super::UINavigationController::CLASS.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIDocument : NSObject ------------------------------------------
pub mod UIDocument {
    use super::*;


    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 8,
        instance_size: 8,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIDocument".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIDocument".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIDocument"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: &raw const NSObject_META,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIDocument"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: &raw const NSObject_CLASS,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIPopoverController : NSObject ---------------------------------
pub mod UIPopoverController {
    use super::*;


    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 8,
        instance_size: 8,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIPopoverController".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIPopoverController".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIPopoverController"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: &raw const NSObject_META,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIPopoverController"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: &raw const NSObject_CLASS,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIRuntimeConnection : NSObject ---------------------------------
pub mod UIRuntimeConnection {
    use super::*;


    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 8,
        instance_size: 8,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIRuntimeConnection".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIRuntimeConnection".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIRuntimeConnection"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: &raw const NSObject_META,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIRuntimeConnection"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: &raw const NSObject_CLASS,
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIRuntimeOutletConnection : UIRuntimeConnection ----------------
pub mod UIRuntimeOutletConnection {
    use super::*;


    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 8,
        instance_size: 8,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIRuntimeOutletConnection".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIRuntimeOutletConnection".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIRuntimeOutletConnection"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: super::UIRuntimeConnection::META.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIRuntimeOutletConnection"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: super::UIRuntimeConnection::CLASS.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIRuntimeOutletCollectionConnection : UIRuntimeConnection ------
pub mod UIRuntimeOutletCollectionConnection {
    use super::*;


    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 8,
        instance_size: 8,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIRuntimeOutletCollectionConnection".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIRuntimeOutletCollectionConnection".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIRuntimeOutletCollectionConnection"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: super::UIRuntimeConnection::META.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIRuntimeOutletCollectionConnection"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: super::UIRuntimeConnection::CLASS.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}

// ---- UIRuntimeEventConnection : UIRuntimeConnection -----------------
pub mod UIRuntimeEventConnection {
    use super::*;


    #[link_section = "__DATA,__objc_const"]
    static RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x80,
        instance_start: 8,
        instance_size: 8,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIRuntimeEventConnection".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[link_section = "__DATA,__objc_const"]
    static META_RO: Writable<ClassRo> = Writable::new(ClassRo {
        flags: 0x81,
        instance_start: META_SIZE,
        instance_size: META_SIZE,
        reserved: 0,
        ivar_layout: std::ptr::null(),
        name: c"UIRuntimeEventConnection".as_ptr(),
        methods: std::ptr::null(),
        protocols: std::ptr::null(),
        ivars: std::ptr::null(),
        weak_ivar_layout: std::ptr::null(),
        properties: std::ptr::null(),
    });
    #[export_name = "OBJC_METACLASS_$_UIRuntimeEventConnection"]
    #[link_section = "__DATA,__objc_data"]
    pub static META: Writable<ClassT> = Writable::new(ClassT {
        isa: &raw const NSObject_META,
        superclass: super::UIRuntimeConnection::META.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: META_RO.ptr(),
    });
    #[export_name = "OBJC_CLASS_$_UIRuntimeEventConnection"]
    #[link_section = "__DATA,__objc_data"]
    pub static CLASS: Writable<ClassT> = Writable::new(ClassT {
        isa: META.ptr(),
        superclass: super::UIRuntimeConnection::CLASS.ptr(),
        cache: &raw const EMPTY_CACHE,
        vtable: std::ptr::null(),
        data: RO.ptr(),
    });
    #[used]
    #[link_section = "__DATA,__objc_classlist,regular,no_dead_strip"]
    static CLASS_LIST: Writable<*const c_void> = Writable::new(CLASS.ptr());
}
