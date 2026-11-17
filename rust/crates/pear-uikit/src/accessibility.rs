//! UIAccessibility, UIPointerInteraction, UITextPosition/UITextRange, and legacy UIUserNotification*.

use pear_objc::{
    cls, msg0, objc_getAssociatedObject, objc_setAssociatedObject, replace_method as add_method,
    sel, CGRect, Class, Id, Imp, Sel, NIL, OBJC_ASSOCIATION_RETAIN,
};
use std::ffi::c_void;

#[no_mangle]
pub extern "C" fn UIAccessibilityIsVoiceOverRunning() -> bool {
    unsafe {
        let ws: Id = msg0(cls("NSWorkspace"), "sharedWorkspace");
        if ws.is_null() {
            false
        } else {
            msg0(ws, "isVoiceOverEnabled")
        }
    }
}

#[no_mangle]
pub extern "C" fn UIAccessibilityPostNotification(_notification: u32, _argument: Id) {}

#[no_mangle]
pub extern "C" fn UIAccessibilityRegisterGestureConflictWithZoom() {}

// ---- UIAccessibilityElement ----
static ACC_CONTAINER_KEY: u8 = 0;

unsafe extern "C" fn acc_init_with_container(self_obj: Id, _cmd: Sel, container: Id) -> Id {
    let obj: Id = msg0(self_obj, "init");
    if !obj.is_null() && !container.is_null() {
        objc_setAssociatedObject(
            obj,
            &ACC_CONTAINER_KEY as *const u8 as *const c_void,
            container,
            OBJC_ASSOCIATION_RETAIN,
        );
    }
    obj
}

unsafe extern "C" fn acc_container(self_obj: Id, _cmd: Sel) -> Id {
    objc_getAssociatedObject(self_obj, &ACC_CONTAINER_KEY as *const u8 as *const c_void)
}

unsafe extern "C" fn acc_is_element(_self_obj: Id, _cmd: Sel) -> bool {
    true
}

// ---- UIPointerInteraction ----
static POINTER_ENABLED_KEY: u8 = 0;

unsafe extern "C" fn pointer_init_with_delegate(self_obj: Id, _cmd: Sel, _del: Id) -> Id {
    let obj: Id = msg0(self_obj, "init");
    if !obj.is_null() {
        objc_setAssociatedObject(
            obj,
            &POINTER_ENABLED_KEY as *const u8 as *const c_void,
            pear_objc::nsnumber_bool(true),
            OBJC_ASSOCIATION_RETAIN,
        );
    }
    obj
}

unsafe extern "C" fn pointer_is_enabled(self_obj: Id, _cmd: Sel) -> bool {
    let v = objc_getAssociatedObject(self_obj, &POINTER_ENABLED_KEY as *const u8 as *const c_void);
    if v.is_null() {
        true
    } else {
        msg0(v, "boolValue")
    }
}

unsafe extern "C" fn pointer_set_enabled(self_obj: Id, _cmd: Sel, enabled: bool) {
    objc_setAssociatedObject(
        self_obj,
        &POINTER_ENABLED_KEY as *const u8 as *const c_void,
        pear_objc::nsnumber_bool(enabled),
        OBJC_ASSOCIATION_RETAIN,
    );
}

unsafe extern "C" fn pointer_invalidate(_self_obj: Id, _cmd: Sel) {}

// ---- UITextRange ----
unsafe extern "C" fn text_range_is_empty(_self_obj: Id, _cmd: Sel) -> bool {
    true
}
unsafe extern "C" fn text_range_start(_self_obj: Id, _cmd: Sel) -> Id {
    msg0(cls("UITextPosition"), "new")
}
unsafe extern "C" fn text_range_end(_self_obj: Id, _cmd: Sel) -> Id {
    msg0(cls("UITextPosition"), "new")
}

// ---- UIUserNotificationSettings ----
static NOTIF_TYPES_KEY: u8 = 0;
static NOTIF_CATS_KEY: u8 = 0;

unsafe extern "C" fn notif_settings_for_types(
    self_cls: Class,
    _cmd: Sel,
    types: usize,
    cats: Id,
) -> Id {
    let obj: Id = msg0(self_cls, "new");
    if !obj.is_null() {
        objc_setAssociatedObject(
            obj,
            &NOTIF_TYPES_KEY as *const u8 as *const c_void,
            pear_objc::nsnumber_i64(types as i64),
            OBJC_ASSOCIATION_RETAIN,
        );
        if !cats.is_null() {
            objc_setAssociatedObject(
                obj,
                &NOTIF_CATS_KEY as *const u8 as *const c_void,
                cats,
                OBJC_ASSOCIATION_RETAIN,
            );
        }
    }
    obj
}

unsafe extern "C" fn notif_types(self_obj: Id, _cmd: Sel) -> usize {
    let v = objc_getAssociatedObject(self_obj, &NOTIF_TYPES_KEY as *const u8 as *const c_void);
    if v.is_null() {
        0
    } else {
        msg0(v, "unsignedIntegerValue")
    }
}

unsafe extern "C" fn notif_cats(self_obj: Id, _cmd: Sel) -> Id {
    objc_getAssociatedObject(self_obj, &NOTIF_CATS_KEY as *const u8 as *const c_void)
}

unsafe extern "C" fn copy_self(self_obj: Id, _cmd: Sel, _zone: *mut c_void) -> Id {
    pear_objc::objc_retain(self_obj)
}

unsafe extern "C" fn ret_zero_rect(_self_obj: Id, _cmd: Sel) -> CGRect {
    CGRect::default()
}

pub unsafe fn register() {
    let acc = cls("UIAccessibilityElement");
    if !acc.is_null() {
        add_method(acc, "initWithAccessibilityContainer:", acc_init_with_container as Imp, "@@:@");
        add_method(acc, "accessibilityContainer", acc_container as Imp, "@@:");
        add_method(acc, "isAccessibilityElement", acc_is_element as Imp, "B@:");
        add_method(acc, "accessibilityFrame", ret_zero_rect as Imp, "{CGRect={CGPoint=dd}{CGSize=dd}}@:");
    }

    let ptr = cls("UIPointerInteraction");
    if !ptr.is_null() {
        add_method(ptr, "initWithDelegate:", pointer_init_with_delegate as Imp, "@@:@");
        add_method(ptr, "isEnabled", pointer_is_enabled as Imp, "B@:");
        add_method(ptr, "setEnabled:", pointer_set_enabled as Imp, "v@:B");
        add_method(ptr, "invalidate", pointer_invalidate as Imp, "v@:");
    }

    let tr = cls("UITextRange");
    if !tr.is_null() {
        add_method(tr, "isEmpty", text_range_is_empty as Imp, "B@:");
        add_method(tr, "start", text_range_start as Imp, "@@:");
        add_method(tr, "end", text_range_end as Imp, "@@:");
    }

    for name in [
        "UIUserNotificationAction",
        "UIMutableUserNotificationAction",
        "UIUserNotificationCategory",
        "UIMutableUserNotificationCategory",
    ] {
        let c = cls(name);
        if !c.is_null() {
            add_method(c, "copyWithZone:", copy_self as Imp, "@@:^{_NSZone=}");
        }
    }

    let uns = cls("UIUserNotificationSettings");
    if !uns.is_null() {
        let meta = pear_objc::object_getClass(uns);
        add_method(meta, "settingsForTypes:categories:", notif_settings_for_types as Imp, "@@:Q@");
        add_method(uns, "types", notif_types as Imp, "Q@:");
        add_method(uns, "categories", notif_cats as Imp, "@@:");
    }
    let _ = sel("init");
    let _ = NIL;
}
