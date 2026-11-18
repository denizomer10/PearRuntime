//! UIAccessibility, UIPointerInteraction, UITextPosition/UITextRange, and legacy UIUserNotification*.

use pear_objc::{
    assoc_bool, assoc_get, assoc_i64, assoc_rect, assoc_set, assoc_set_bool,
    assoc_set_copy, assoc_set_i64, assoc_set_rect, autoreleased, cls, msg0, msg1,
    replace_method as add_method, CGRect, Class, Id, Imp, Sel,
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
static ACC_IS_ELEMENT_KEY: u8 = 0;
static ACC_LABEL_KEY: u8 = 0;
static ACC_HINT_KEY: u8 = 0;
static ACC_VALUE_KEY: u8 = 0;
static ACC_FRAME_KEY: u8 = 0;
static ACC_TRAITS_KEY: u8 = 0;

unsafe extern "C" fn acc_init_with_container(self_obj: Id, _cmd: Sel, container: Id) -> Id {
    let obj: Id = msg0(self_obj, "init");
    // The container owns its elements; the back pointer is not retained.
    pear_objc::assoc_set_weak(obj, &ACC_CONTAINER_KEY, container);
    obj
}
unsafe extern "C" fn acc_container(self_obj: Id, _cmd: Sel) -> Id {
    pear_objc::assoc_get_weak(self_obj, &ACC_CONTAINER_KEY)
}
unsafe extern "C" fn acc_set_container(self_obj: Id, _cmd: Sel, c: Id) {
    pear_objc::assoc_set_weak(self_obj, &ACC_CONTAINER_KEY, c);
}
unsafe extern "C" fn acc_is_element(self_obj: Id, _cmd: Sel) -> bool {
    assoc_bool(self_obj, &ACC_IS_ELEMENT_KEY, true)
}
unsafe extern "C" fn acc_set_is_element(self_obj: Id, _cmd: Sel, v: bool) {
    assoc_set_bool(self_obj, &ACC_IS_ELEMENT_KEY, v);
}
unsafe extern "C" fn acc_label(self_obj: Id, _cmd: Sel) -> Id {
    assoc_get(self_obj, &ACC_LABEL_KEY)
}
unsafe extern "C" fn acc_set_label(self_obj: Id, _cmd: Sel, v: Id) {
    assoc_set_copy(self_obj, &ACC_LABEL_KEY, v);
}
unsafe extern "C" fn acc_hint(self_obj: Id, _cmd: Sel) -> Id {
    assoc_get(self_obj, &ACC_HINT_KEY)
}
unsafe extern "C" fn acc_set_hint(self_obj: Id, _cmd: Sel, v: Id) {
    assoc_set_copy(self_obj, &ACC_HINT_KEY, v);
}
unsafe extern "C" fn acc_value(self_obj: Id, _cmd: Sel) -> Id {
    assoc_get(self_obj, &ACC_VALUE_KEY)
}
unsafe extern "C" fn acc_set_value(self_obj: Id, _cmd: Sel, v: Id) {
    assoc_set_copy(self_obj, &ACC_VALUE_KEY, v);
}
unsafe extern "C" fn acc_frame(self_obj: Id, _cmd: Sel) -> CGRect {
    assoc_rect(self_obj, &ACC_FRAME_KEY)
}
unsafe extern "C" fn acc_set_frame(self_obj: Id, _cmd: Sel, r: CGRect) {
    assoc_set_rect(self_obj, &ACC_FRAME_KEY, r);
}
unsafe extern "C" fn acc_traits(self_obj: Id, _cmd: Sel) -> u64 {
    assoc_i64(self_obj, &ACC_TRAITS_KEY, 0) as u64
}
unsafe extern "C" fn acc_set_traits(self_obj: Id, _cmd: Sel, v: u64) {
    assoc_set_i64(self_obj, &ACC_TRAITS_KEY, v as i64);
}

// ---- UIPointerInteraction ----
static POINTER_ENABLED_KEY: u8 = 0;
static POINTER_DELEGATE_KEY: u8 = 0;
static POINTER_VIEW_KEY: u8 = 0;

unsafe extern "C" fn pointer_init_with_delegate(self_obj: Id, _cmd: Sel, del: Id) -> Id {
    let obj: Id = msg0(self_obj, "init");
    pear_objc::assoc_set_weak(obj, &POINTER_DELEGATE_KEY, del);
    obj
}
unsafe extern "C" fn pointer_is_enabled(self_obj: Id, _cmd: Sel) -> bool {
    assoc_bool(self_obj, &POINTER_ENABLED_KEY, true)
}
unsafe extern "C" fn pointer_set_enabled(self_obj: Id, _cmd: Sel, enabled: bool) {
    assoc_set_bool(self_obj, &POINTER_ENABLED_KEY, enabled);
}
unsafe extern "C" fn pointer_delegate(self_obj: Id, _cmd: Sel) -> Id {
    pear_objc::assoc_get_weak(self_obj, &POINTER_DELEGATE_KEY)
}
unsafe extern "C" fn pointer_set_delegate(self_obj: Id, _cmd: Sel, d: Id) {
    pear_objc::assoc_set_weak(self_obj, &POINTER_DELEGATE_KEY, d);
}
unsafe extern "C" fn pointer_view(self_obj: Id, _cmd: Sel) -> Id {
    pear_objc::assoc_get_weak(self_obj, &POINTER_VIEW_KEY)
}
// UIInteraction: the view calls this from addInteraction:/removeInteraction:.
unsafe extern "C" fn pointer_will_move_to_view(_self_obj: Id, _cmd: Sel, _v: Id) {}
unsafe extern "C" fn pointer_did_move_to_view(self_obj: Id, _cmd: Sel, v: Id) {
    pear_objc::assoc_set_weak(self_obj, &POINTER_VIEW_KEY, v);
}
unsafe extern "C" fn pointer_invalidate(_self_obj: Id, _cmd: Sel) {}

// ---- UITextRange ----
unsafe extern "C" fn text_range_is_empty(_self_obj: Id, _cmd: Sel) -> bool {
    true
}
unsafe extern "C" fn text_range_start(_self_obj: Id, _cmd: Sel) -> Id {
    autoreleased(msg0(cls("UITextPosition"), "new"))
}
unsafe extern "C" fn text_range_end(_self_obj: Id, _cmd: Sel) -> Id {
    autoreleased(msg0(cls("UITextPosition"), "new"))
}

// ---- UIUserNotificationAction / Category ----
static ACTION_ID_KEY: u8 = 0;
static ACTION_TITLE_KEY: u8 = 0;
static ACTION_MODE_KEY: u8 = 0;
static ACTION_AUTH_KEY: u8 = 0;
static ACTION_DESTRUCTIVE_KEY: u8 = 0;
static CATEGORY_ID_KEY: u8 = 0;
static CATEGORY_ACTIONS_KEY: u8 = 0;

unsafe extern "C" fn action_identifier(s: Id, _c: Sel) -> Id {
    assoc_get(s, &ACTION_ID_KEY)
}
unsafe extern "C" fn action_set_identifier(s: Id, _c: Sel, v: Id) {
    assoc_set_copy(s, &ACTION_ID_KEY, v);
}
unsafe extern "C" fn action_title(s: Id, _c: Sel) -> Id {
    assoc_get(s, &ACTION_TITLE_KEY)
}
unsafe extern "C" fn action_set_title(s: Id, _c: Sel, v: Id) {
    assoc_set_copy(s, &ACTION_TITLE_KEY, v);
}
unsafe extern "C" fn action_mode(s: Id, _c: Sel) -> usize {
    assoc_i64(s, &ACTION_MODE_KEY, 0) as usize
}
unsafe extern "C" fn action_set_mode(s: Id, _c: Sel, v: usize) {
    assoc_set_i64(s, &ACTION_MODE_KEY, v as i64);
}
unsafe extern "C" fn action_auth(s: Id, _c: Sel) -> bool {
    assoc_bool(s, &ACTION_AUTH_KEY, false)
}
unsafe extern "C" fn action_set_auth(s: Id, _c: Sel, v: bool) {
    assoc_set_bool(s, &ACTION_AUTH_KEY, v);
}
unsafe extern "C" fn action_destructive(s: Id, _c: Sel) -> bool {
    assoc_bool(s, &ACTION_DESTRUCTIVE_KEY, false)
}
unsafe extern "C" fn action_set_destructive(s: Id, _c: Sel, v: bool) {
    assoc_set_bool(s, &ACTION_DESTRUCTIVE_KEY, v);
}
unsafe fn action_copy_into(src: Id, dst: Id) {
    for key in [&ACTION_ID_KEY, &ACTION_TITLE_KEY, &ACTION_MODE_KEY, &ACTION_AUTH_KEY, &ACTION_DESTRUCTIVE_KEY] {
        assoc_set(dst, key, assoc_get(src, key));
    }
}
unsafe extern "C" fn action_copy(s: Id, _c: Sel, _zone: *mut c_void) -> Id {
    // Immutable actions are shared; a mutable one copies into an immutable one.
    if pear_objc::object_getClass(s) == cls("UIUserNotificationAction") {
        return pear_objc::objc_retain(s);
    }
    let copy: Id = msg0(cls("UIUserNotificationAction"), "new");
    action_copy_into(s, copy);
    copy
}
unsafe extern "C" fn action_mutable_copy(s: Id, _c: Sel, _zone: *mut c_void) -> Id {
    let copy: Id = msg0(cls("UIMutableUserNotificationAction"), "new");
    action_copy_into(s, copy);
    copy
}

unsafe fn category_actions(s: Id) -> Id {
    let mut d = assoc_get(s, &CATEGORY_ACTIONS_KEY);
    if d.is_null() {
        d = msg0(cls("NSMutableDictionary"), "dictionary");
        assoc_set(s, &CATEGORY_ACTIONS_KEY, d);
    }
    d
}
unsafe extern "C" fn category_identifier(s: Id, _c: Sel) -> Id {
    assoc_get(s, &CATEGORY_ID_KEY)
}
unsafe extern "C" fn category_set_identifier(s: Id, _c: Sel, v: Id) {
    assoc_set_copy(s, &CATEGORY_ID_KEY, v);
}
unsafe extern "C" fn category_actions_for_context(s: Id, _c: Sel, ctx: usize) -> Id {
    msg1(category_actions(s), "objectForKey:", pear_objc::nsnumber_i64(ctx as i64))
}
unsafe extern "C" fn category_set_actions(s: Id, _c: Sel, actions: Id, ctx: usize) {
    let key = pear_objc::nsnumber_i64(ctx as i64);
    let d = category_actions(s);
    if actions.is_null() {
        msg1::<Id, ()>(d, "removeObjectForKey:", key);
    } else {
        let copy: Id = msg0(actions, "copy");
        pear_objc::msg2::<Id, Id, ()>(d, "setObject:forKey:", copy, key);
        pear_objc::objc_release(copy);
    }
}
unsafe fn category_copy_into(src: Id, dst: Id) {
    assoc_set(dst, &CATEGORY_ID_KEY, assoc_get(src, &CATEGORY_ID_KEY));
    let d = assoc_get(src, &CATEGORY_ACTIONS_KEY);
    if !d.is_null() {
        let copy: Id = msg0(d, "mutableCopy");
        assoc_set(dst, &CATEGORY_ACTIONS_KEY, copy);
        pear_objc::objc_release(copy);
    }
}
unsafe extern "C" fn category_copy(s: Id, _c: Sel, _zone: *mut c_void) -> Id {
    if pear_objc::object_getClass(s) == cls("UIUserNotificationCategory") {
        return pear_objc::objc_retain(s);
    }
    let copy: Id = msg0(cls("UIUserNotificationCategory"), "new");
    category_copy_into(s, copy);
    copy
}
unsafe extern "C" fn category_mutable_copy(s: Id, _c: Sel, _zone: *mut c_void) -> Id {
    let copy: Id = msg0(cls("UIMutableUserNotificationCategory"), "new");
    category_copy_into(s, copy);
    copy
}

// ---- UIUserNotificationSettings ----
static NOTIF_TYPES_KEY: u8 = 0;
static NOTIF_CATS_KEY: u8 = 0;

unsafe extern "C" fn notif_settings_for_types(self_cls: Class, _cmd: Sel, types: usize, cats: Id) -> Id {
    let obj: Id = msg0(self_cls, "new");
    assoc_set_i64(obj, &NOTIF_TYPES_KEY, types as i64);
    if !cats.is_null() {
        let copy: Id = msg0(cats, "copy");
        assoc_set(obj, &NOTIF_CATS_KEY, copy);
        pear_objc::objc_release(copy);
    }
    autoreleased(obj)
}
unsafe extern "C" fn notif_types(self_obj: Id, _cmd: Sel) -> usize {
    assoc_i64(self_obj, &NOTIF_TYPES_KEY, 0) as usize
}
unsafe extern "C" fn notif_set_types(self_obj: Id, _cmd: Sel, t: usize) {
    assoc_set_i64(self_obj, &NOTIF_TYPES_KEY, t as i64);
}
unsafe extern "C" fn notif_cats(self_obj: Id, _cmd: Sel) -> Id {
    assoc_get(self_obj, &NOTIF_CATS_KEY)
}
unsafe extern "C" fn notif_set_cats(self_obj: Id, _cmd: Sel, c: Id) {
    assoc_set_copy(self_obj, &NOTIF_CATS_KEY, c);
}

const RECT: &str = "{CGRect={CGPoint=dd}{CGSize=dd}}";

pub unsafe fn register() {
    let acc = cls("UIAccessibilityElement");
    if !acc.is_null() {
        add_method(acc, "initWithAccessibilityContainer:", acc_init_with_container as Imp, "@@:@");
        add_method(acc, "accessibilityContainer", acc_container as Imp, "@@:");
        add_method(acc, "setAccessibilityContainer:", acc_set_container as Imp, "v@:@");
        add_method(acc, "isAccessibilityElement", acc_is_element as Imp, "B@:");
        add_method(acc, "setIsAccessibilityElement:", acc_set_is_element as Imp, "v@:B");
        add_method(acc, "accessibilityLabel", acc_label as Imp, "@@:");
        add_method(acc, "setAccessibilityLabel:", acc_set_label as Imp, "v@:@");
        add_method(acc, "accessibilityHint", acc_hint as Imp, "@@:");
        add_method(acc, "setAccessibilityHint:", acc_set_hint as Imp, "v@:@");
        add_method(acc, "accessibilityValue", acc_value as Imp, "@@:");
        add_method(acc, "setAccessibilityValue:", acc_set_value as Imp, "v@:@");
        add_method(acc, "accessibilityFrame", acc_frame as Imp, &format!("{RECT}@:"));
        add_method(acc, "setAccessibilityFrame:", acc_set_frame as Imp, &format!("v@:{RECT}"));
        add_method(acc, "accessibilityTraits", acc_traits as Imp, "Q@:");
        add_method(acc, "setAccessibilityTraits:", acc_set_traits as Imp, "v@:Q");
    }

    let ptr = cls("UIPointerInteraction");
    if !ptr.is_null() {
        add_method(ptr, "initWithDelegate:", pointer_init_with_delegate as Imp, "@@:@");
        add_method(ptr, "isEnabled", pointer_is_enabled as Imp, "B@:");
        add_method(ptr, "setEnabled:", pointer_set_enabled as Imp, "v@:B");
        add_method(ptr, "delegate", pointer_delegate as Imp, "@@:");
        add_method(ptr, "setDelegate:", pointer_set_delegate as Imp, "v@:@");
        add_method(ptr, "view", pointer_view as Imp, "@@:");
        add_method(ptr, "willMoveToView:", pointer_will_move_to_view as Imp, "v@:@");
        add_method(ptr, "didMoveToView:", pointer_did_move_to_view as Imp, "v@:@");
        add_method(ptr, "invalidate", pointer_invalidate as Imp, "v@:");
    }

    let tr = cls("UITextRange");
    if !tr.is_null() {
        add_method(tr, "isEmpty", text_range_is_empty as Imp, "B@:");
        add_method(tr, "start", text_range_start as Imp, "@@:");
        add_method(tr, "end", text_range_end as Imp, "@@:");
    }

    let act = cls("UIUserNotificationAction");
    if !act.is_null() {
        add_method(act, "identifier", action_identifier as Imp, "@@:");
        add_method(act, "setIdentifier:", action_set_identifier as Imp, "v@:@");
        add_method(act, "title", action_title as Imp, "@@:");
        add_method(act, "setTitle:", action_set_title as Imp, "v@:@");
        add_method(act, "activationMode", action_mode as Imp, "Q@:");
        add_method(act, "setActivationMode:", action_set_mode as Imp, "v@:Q");
        add_method(act, "isAuthenticationRequired", action_auth as Imp, "B@:");
        add_method(act, "setAuthenticationRequired:", action_set_auth as Imp, "v@:B");
        add_method(act, "setIsAuthenticationRequired:", action_set_auth as Imp, "v@:B");
        add_method(act, "isDestructive", action_destructive as Imp, "B@:");
        add_method(act, "setDestructive:", action_set_destructive as Imp, "v@:B");
        add_method(act, "setIsDestructive:", action_set_destructive as Imp, "v@:B");
        add_method(act, "copyWithZone:", action_copy as Imp, "@@:^{_NSZone=}");
        add_method(act, "mutableCopyWithZone:", action_mutable_copy as Imp, "@@:^{_NSZone=}");
    }

    let cat = cls("UIUserNotificationCategory");
    if !cat.is_null() {
        add_method(cat, "identifier", category_identifier as Imp, "@@:");
        add_method(cat, "setIdentifier:", category_set_identifier as Imp, "v@:@");
        add_method(cat, "actionsForContext:", category_actions_for_context as Imp, "@@:Q");
        add_method(cat, "setActions:forContext:", category_set_actions as Imp, "v@:@Q");
        add_method(cat, "copyWithZone:", category_copy as Imp, "@@:^{_NSZone=}");
        add_method(cat, "mutableCopyWithZone:", category_mutable_copy as Imp, "@@:^{_NSZone=}");
    }

    let uns = cls("UIUserNotificationSettings");
    if !uns.is_null() {
        let meta = pear_objc::object_getClass(uns);
        add_method(meta, "settingsForTypes:categories:", notif_settings_for_types as Imp, "@@:Q@");
        add_method(uns, "types", notif_types as Imp, "Q@:");
        add_method(uns, "setTypes:", notif_set_types as Imp, "v@:Q");
        add_method(uns, "categories", notif_cats as Imp, "@@:");
        add_method(uns, "setCategories:", notif_set_cats as Imp, "v@:@");
    }
}
