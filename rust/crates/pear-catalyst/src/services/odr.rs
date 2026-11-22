//! On-Demand Resources resolve to the bundles shipped inside the app instead
//! of the (nonexistent) ODR service.
//!
//! Unity titles built with Addressables over ODR keep their content in
//! `Data/Raw/aa/BuildTarget/*.bundle` and ask NSBundleResourceRequest for each
//! tag first; on a Mac every request fails (Cocoa 4994) and the game stops at
//! a black window. Requests are answered locally: available when the package
//! carries the tag's bundle, and the missing ones are named in the log.
//! External packs can live in ~/Library/Application Support/PearSilicon/ODR/<id>
//! (or PEAR_ODR_PATH). Disable with PEAR_NO_ODR_SHIM=1.

use crate::rt;
use pear_objc::{cls, msg0, msg1, msg2, msg3, nsarray_count, nsarray_object_at, nsstring, objc_retain, replace_method, Id, Imp, Sel, NIL};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::Mutex;

static LOCAL_BUNDLE: Mutex<usize> = Mutex::new(0);
static PROGRESS: Mutex<usize> = Mutex::new(0);
static QUEUE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static REPORTED_MISSING: AtomicBool = AtomicBool::new(false);

extern "C" {
    fn NSTemporaryDirectory() -> Id;
    fn NSHomeDirectory() -> Id;
    fn getpid() -> i32;
}

unsafe fn app_path() -> String {
    rt::ns_string(msg0(msg0::<Id>(cls("NSBundle"), "mainBundle"), "bundlePath"))
}

/// Optional per-app directory for packs that are not part of the package.
unsafe fn external_root() -> Option<String> {
    if let Ok(o) = std::env::var("PEAR_ODR_PATH") {
        if !o.is_empty() {
            let mut path = rt::ns_string(msg0(nsstring(&o), "stringByExpandingTildeInPath"));
            if !path.starts_with('/') {
                let cwd = rt::ns_string(msg0(rt::file_manager(), "currentDirectoryPath"));
                path = rt::path_join(&cwd, &path);
            }
            return Some(rt::ns_string(msg0(nsstring(&path), "stringByStandardizingPath")));
        }
    }
    let bundle_id: Id = msg0(msg0::<Id>(cls("NSBundle"), "mainBundle"), "bundleIdentifier");
    if bundle_id.is_null() || msg0::<usize>(bundle_id, "length") == 0 {
        return None;
    }
    let id = rt::ns_string(bundle_id);
    // PEAR_SUPPORT_ROOT follows relocated storage; NSHomeDirectory is the
    // (contained) one PathShim reports, as before.
    match std::env::var("PEAR_SUPPORT_ROOT") {
        Ok(r) if !r.is_empty() => Some(rt::path_join(&rt::path_join(&r, "ODR"), &id)),
        _ => Some(rt::path_join(&rt::path_join(&rt::ns_string(NSHomeDirectory()), "Library/Application Support/PearSilicon/ODR"), &id)),
    }
}

/// A read-only view of the app's own bundles plus the external ones
/// (symlinks; copies only where symlinks are refused).
unsafe fn merge_external_root(local: &str, external: &str) -> String {
    let fm = rt::file_manager();
    let mut sources = vec![local.to_string(), external.to_string()];
    // A flat directory of bundles, or a copy of the tree with BuildTarget.
    let nested = rt::path_join(external, "BuildTarget");
    if rt::is_directory(&nested) {
        sources.push(nested);
    }
    let local_count = rt::directory_entries(local).len();
    let external_count: usize = sources.iter().filter(|s| s.as_str() != local).map(|s| rt::directory_entries(s).len()).sum();
    if external_count == 0 {
        return local.to_string();
    }
    let staging = rt::path_join(&rt::ns_string(NSTemporaryDirectory()), &format!("PearODR-{}", getpid()));
    msg2::<Id, *mut Id, bool>(fm, "removeItemAtPath:error:", nsstring(&staging), std::ptr::null_mut());
    if !pear_objc::msg4::<Id, bool, Id, *mut Id, bool>(fm, "createDirectoryAtPath:withIntermediateDirectories:attributes:error:", nsstring(&staging), true, NIL, std::ptr::null_mut()) {
        rt::log(&format!("[PearRuntime][odr] could not create merged root at {staging}"));
        return local.to_string();
    }
    let mut names = std::collections::HashSet::new();
    let (mut linked, mut copied) = (0usize, 0usize);
    for source in &sources {
        for name in rt::directory_entries(source) {
            if names.contains(&name) {
                continue;
            }
            let from = nsstring(&rt::path_join(source, &name));
            let to = nsstring(&rt::path_join(&staging, &name));
            let mut error: Id = NIL;
            if msg3::<Id, Id, *mut Id, bool>(fm, "createSymbolicLinkAtPath:withDestinationPath:error:", to, from, &mut error) {
                names.insert(name);
                linked += 1;
            } else if msg3::<Id, Id, *mut Id, bool>(fm, "copyItemAtPath:toPath:error:", from, to, std::ptr::null_mut()) {
                names.insert(name);
                copied += 1;
            } else {
                let why = if error.is_null() { "unknown error".into() } else { rt::ns_string(msg0(error, "localizedDescription")) };
                rt::log(&format!("[PearRuntime][odr] could not merge {name}: {why}"));
            }
        }
    }
    rt::log(&format!(
        "[PearRuntime][odr] external root {external}: {local_count} local and {external_count} external entries merged into {staging} ({linked} links, {copied} copies)"
    ));
    staging
}

/// The NSBundle standing in for the asset pack.
unsafe fn local_bundle() -> Id {
    let mut slot = LOCAL_BUNDLE.lock().unwrap_or_else(|e| e.into_inner());
    if *slot != 0 {
        return *slot as Id;
    }
    let app = app_path();
    let local = ["Data/Raw/aa/BuildTarget", "Data/Raw/aa", "Data/Raw"]
        .iter()
        .map(|p| rt::path_join(&app, p))
        .find(|p| rt::is_directory(p))
        .unwrap_or_else(|| app.clone());
    let mut root = local.clone();
    if let Some(external) = external_root() {
        if !external.is_empty() && rt::path_exists(&external) {
            root = merge_external_root(&local, &external);
        }
    }
    let mut bundle: Id = msg1(cls("NSBundle"), "bundleWithPath:", nsstring(&root));
    if bundle.is_null() {
        bundle = msg1(cls("NSBundle"), "bundleWithPath:", nsstring(&app));
    }
    *slot = objc_retain(bundle) as usize;
    drop(slot);
    rt::log(&format!("[PearRuntime][odr] local resource root: {}", bundle_path()));
    bundle
}

unsafe fn bundle_path() -> String {
    let b = local_bundle();
    if b.is_null() { String::new() } else { rt::ns_string(msg0(b, "bundlePath")) }
}

/// `<tag>_assets_all.bundle` (Addressables), `<tag>.bundle`, or a directory.
unsafe fn local_path_for_tag(tag: &str) -> Option<String> {
    let root = bundle_path();
    [format!("{tag}_assets_all.bundle"), format!("{tag}.bundle"), tag.to_string()]
        .iter()
        .map(|n| rt::path_join(&root, n))
        .find(|p| rt::path_exists(p))
}

unsafe fn manifest_tags() -> Id {
    let manifest = rt::path_join(&app_path(), "OnDemandResources.plist");
    let plist: Id = msg1(cls("NSDictionary"), "dictionaryWithContentsOfFile:", nsstring(&manifest));
    if plist.is_null() || !pear_objc::is_kind_of(plist, "NSDictionary") {
        return NIL;
    }
    msg1(plist, "objectForKey:", nsstring("NSBundleResourceRequestTags"))
}

/// Warns once when the app declares ODR tags but carries no bundles for them.
unsafe fn check_for_missing_content() {
    if REPORTED_MISSING.swap(true, Ordering::AcqRel) {
        return;
    }
    let manifest = rt::path_join(&app_path(), "OnDemandResources.plist");
    let plist: Id = msg1(cls("NSDictionary"), "dictionaryWithContentsOfFile:", nsstring(&manifest));
    if plist.is_null() || !pear_objc::is_kind_of(plist, "NSDictionary") {
        return;
    }
    let root = bundle_path();
    let bundles = rt::directory_entries(&root).iter().filter(|e| e.rsplit_once('.').map(|(_, x)| x) == Some("bundle")).count();
    if bundles == 0 {
        rt::log("[PearRuntime][odr] WARNING: this app uses On-Demand Resources but the app package contains no asset bundles. The game will stop loading (black window). Use an app package that includes the ODR asset packs.");
        return;
    }
    let tags = manifest_tags();
    let keys: Id = if tags.is_null() { NIL } else { msg0(tags, "allKeys") };
    let mut missing = Vec::new();
    for i in 0..nsarray_count(keys) {
        let tag = nsarray_object_at(keys, i);
        if pear_objc::is_kind_of(tag, "NSString") && local_path_for_tag(&rt::ns_string(tag)).is_none() {
            missing.push(rt::ns_string(tag));
        }
    }
    if missing.is_empty() {
        rt::log(&format!("[PearRuntime][odr] {bundles} local bundle(s) available for ODR requests"));
        return;
    }
    let shown = missing.len().min(8);
    rt::log(&format!(
        "[PearRuntime][odr] WARNING: {} of {} ODR tag(s) have no local asset bundle: {}{}. Expected {}_assets_all.bundle under {root}. Loading will stop at the first missing bundle; the app package must include its On-Demand Resources.",
        missing.len(),
        nsarray_count(keys),
        missing[..shown].join(", "),
        if missing.len() > shown { ", ..." } else { "" },
        missing[0]
    ));
}

unsafe fn asset_packs_for_tag(tag: &str) -> Vec<String> {
    let tags = manifest_tags();
    if tags.is_null() || !pear_objc::is_kind_of(tags, "NSDictionary") {
        return vec![];
    }
    let entry: Id = msg1(tags, "objectForKey:", nsstring(tag));
    if entry.is_null() || !pear_objc::is_kind_of(entry, "NSDictionary") {
        return vec![];
    }
    let packs: Id = msg1(entry, "objectForKey:", nsstring("NSAssetPacks"));
    if packs.is_null() || !pear_objc::is_kind_of(packs, "NSArray") {
        return vec![];
    }
    (0..nsarray_count(packs)).map(|i| rt::description(nsarray_object_at(packs, i))).collect()
}

unsafe fn report_missing_tag(tag: &str) {
    if local_path_for_tag(tag).is_some() {
        return;
    }
    let packs = asset_packs_for_tag(tag);
    let list = if packs.is_empty() { "none listed".to_string() } else { packs.join(", ") };
    rt::log(&format!(
        "[PearRuntime][odr] WARNING: tag '{tag}' has no local bundle; expected {tag}_assets_all.bundle in {} (the app package is missing its On-Demand Resources; asset pack(s): {list})",
        bundle_path()
    ));
}

unsafe fn tag_strings(request: Id) -> (Id, Vec<String>) {
    let tags: Id = msg0(request, "tags");
    let all: Id = if tags.is_null() { NIL } else { msg0(tags, "allObjects") };
    let list = (0..nsarray_count(all))
        .map(|i| nsarray_object_at(all, i))
        .filter(|t| pear_objc::is_kind_of(*t, "NSString"))
        .map(|t| rt::ns_string(t))
        .collect();
    (tags, list)
}

/// A tag without a local bundle is not reported available: apps that trust
/// the answer enter their game loop with data that was never downloaded.
unsafe fn tags_available(tags: &[String]) -> bool {
    tags.iter().all(|t| local_path_for_tag(t).is_some())
}

unsafe fn call_error_handler(block: Id, error: usize) {
    if let Some(f) = rt::block_invoke::<unsafe extern "C" fn(Id, Id)>(block) {
        f(block, error as Id);
    }
    if error != 0 {
        pear_objc::objc_release(error as Id);
    }
}

unsafe fn call_bool_handler(block: Id, value: usize) {
    if let Some(f) = rt::block_invoke::<unsafe extern "C" fn(Id, bool)>(block) {
        f(block, value != 0);
    }
}

unsafe extern "C" fn begin(s: Id, _c: Sel, handler: Id) {
    check_for_missing_content();
    let (tags, list) = tag_strings(s);
    for t in &list {
        report_missing_tag(t);
    }
    let available = tags_available(&list);
    rt::log(&format!(
        "[PearRuntime][odr] begin tags={} -> {} ({})",
        rt::description(tags),
        if available { "available" } else { "missing" },
        bundle_path()
    ));
    if handler.is_null() {
        return;
    }
    let error: Id = if available {
        NIL
    } else {
        let info: Id = msg2(cls("NSDictionary"), "dictionaryWithObject:forKey:", nsstring("The requested application data doesn't exist."), nsstring("NSLocalizedDescription"));
        objc_retain(msg3(cls("NSError"), "errorWithDomain:code:userInfo:", nsstring("NSCocoaErrorDomain"), 4994isize, info))
    };
    rt::async_on_with_block(QUEUE.load(Ordering::Acquire), handler, error as usize, call_error_handler);
}

unsafe extern "C" fn conditional(s: Id, _c: Sel, handler: Id) {
    let (tags, list) = tag_strings(s);
    let available = tags_available(&list);
    rt::log(&format!("[PearRuntime][odr] conditional tags={} -> {}", rt::description(tags), if available { "YES" } else { "NO" }));
    if handler.is_null() {
        return;
    }
    rt::async_on_with_block(QUEUE.load(Ordering::Acquire), handler, available as usize, call_bool_handler);
}

unsafe extern "C" fn bundle(_s: Id, _c: Sel) -> Id {
    local_bundle()
}

unsafe extern "C" fn progress(_s: Id, _c: Sel) -> Id {
    let mut slot = PROGRESS.lock().unwrap_or_else(|e| e.into_inner());
    if *slot == 0 {
        let p: Id = msg1(cls("NSProgress"), "progressWithTotalUnitCount:", 1i64);
        msg1::<i64, ()>(p, "setCompletedUnitCount:", 1);
        *slot = objc_retain(p) as usize;
    }
    *slot as Id
}

unsafe extern "C" fn end(s: Id, _c: Sel) {
    rt::log(&format!("[PearRuntime][odr] end tags={}", rt::description(msg0(s, "tags"))));
}

// NSBundle's preservation-priority API throws when the bundle has no ODR tag
// information, which is exactly this case.
unsafe extern "C" fn set_preservation(_s: Id, _c: Sel, _priority: f64, _tags: Id) {}
unsafe extern "C" fn get_preservation(_s: Id, _c: Sel, _tag: Id) -> f64 {
    0.0
}

pub fn init() {
    if !rt::module_enabled("services") || rt::env_set("PEAR_NO_ODR_SHIM") {
        return;
    }
    unsafe {
        let request = cls("NSBundleResourceRequest");
        if request.is_null() {
            rt::log("[PearRuntime][odr] NSBundleResourceRequest not found");
            return;
        }
        QUEUE.store(rt::serial_queue(c"com.pearsilicon.odr"), Ordering::Release);
        replace_method(request, "beginAccessingResourcesWithCompletionHandler:", begin as Imp, "v@:@?");
        replace_method(request, "conditionallyBeginAccessingResourcesWithCompletionHandler:", conditional as Imp, "v@:@?");
        replace_method(request, "bundle", bundle as Imp, "@@:");
        replace_method(request, "progress", progress as Imp, "@@:");
        replace_method(request, "endAccessingResources", end as Imp, "v@:");
        let ns_bundle = cls("NSBundle");
        if !ns_bundle.is_null() {
            replace_method(ns_bundle, "setPreservationPriority:forTags:", set_preservation as Imp, "v@:d@");
            replace_method(ns_bundle, "preservationPriorityForTag:", get_preservation as Imp, "d@:@");
        }
        rt::log(&format!("[PearRuntime][odr] shim installed on {}", rt::class_name(request)));
    }
}
