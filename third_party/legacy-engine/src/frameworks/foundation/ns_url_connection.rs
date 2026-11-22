/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSURLConnection`.
//!
//! Network access is not implemented, but the class still has to follow the
//! real Foundation contract: creating a connection returns a live object that
//! reports the failure to its delegate. Guest apps (e.g. Asphalt 6) store the
//! connection, subscribe to its delegate callbacks and dereference it later, so
//! returning `nil` from the initializer corrupts their state and crashes them.

use super::ns_run_loop::add_perform_request;
use super::{ns_string, NSInteger};
use crate::environment::Environment;
use crate::mem::MutPtr;
use crate::objc::{
    autorelease, id, msg, msg_class, nil, objc_classes, release, retain, ClassExports, HostObject,
    NSZonePtr, SEL,
};
use std::borrow::Cow;

const NSURLErrorDomain: &str = "NSURLErrorDomain";

/// Our helper type, Foundation just uses ints.
type NSURLErrorCode = NSInteger;
const NSURLErrorNotConnectedToInternet: NSURLErrorCode = -1009;

/// Selector used to deliver the (always failing) result through the run loop,
/// so the delegate is never called from inside the initializer.
const DELIVER_FAILURE_SELECTOR: &str = "pearDeliverFailure:";

struct NSURLConnectionHostObject {
    /// `NSURLRequest*`
    request: id,
    /// Real `NSURLConnection` retains its delegate until the connection
    /// finishes, fails or is cancelled, and guest apps rely on that (Asphalt 6
    /// passes an autoreleased delegate and breaks if it dies too early).
    delegate: id,
    started: bool,
    cancelled: bool,
}
impl HostObject for NSURLConnectionHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSURLConnection: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(NSURLConnectionHostObject {
        request: nil,
        delegate: nil,
        started: false,
        cancelled: false,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

+ (id)sendSynchronousRequest:(id)request // NSURLRequest *
           returningResponse:(MutPtr<id>)response // NSURLResponse **
                       error:(MutPtr<id>)out_error { // NSError **
    log!(
        "TODO: [NSURLConnection sendSynchronousRequest:{:?} ('{}') response:{:?} error:{:?}] -> nil",
        request,
        url_string_from_request(env, request),
        response,
        out_error,
    );
    if !response.is_null() {
        env.mem.write(response, nil);
    }
    if !out_error.is_null() {
        let error = make_not_connected_error(env);
        env.mem.write(out_error, error);
    }
    nil
}

+ (id)connectionWithRequest:(id)request // NSURLRequest *
                   delegate:(id)delegate {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithRequest:request delegate:delegate];
    autorelease(env, new)
}

- (id)initWithRequest:(id)request // NSURLRequest *
             delegate:(id)delegate {
    msg![env; this initWithRequest:request delegate:delegate startImmediately:true]
}

- (id)initWithRequest:(id)request // NSURLRequest *
             delegate:(id)delegate
     startImmediately:(bool)start_immediately {
    log!(
        "TODO: [(NSURLConnection *){:?} initWithRequest:{:?} ('{}') delegate:{:?} startImmediately:{}] (network access unavailable, will report a failure to the delegate)",
        this,
        request,
        url_string_from_request(env, request),
        delegate,
        start_immediately,
    );

    retain(env, request);
    retain(env, delegate);
    let (old_request, old_delegate) = {
        let host_object = env.objc.borrow_mut::<NSURLConnectionHostObject>(this);
        let old_request = host_object.request;
        let old_delegate = host_object.delegate;
        host_object.request = request;
        host_object.delegate = delegate;
        (old_request, old_delegate)
    };
    release(env, old_request);
    release(env, old_delegate);

    if start_immediately {
        schedule_failure(env, this);
    }

    this
}

- (id)initWithRequest:(id)request
             delegate:(id)delegate
     startImmediately:(bool)start_immediately
              runLoop:(id)run_loop // NSRunLoop *
                 mode:(id)run_loop_mode { // NSString *
    log_dbg!(
        "[(NSURLConnection *){:?} initWithRequest:... runLoop:{:?} mode:{:?}] ignoring custom run loop",
        this,
        run_loop,
        run_loop_mode,
    );
    msg![env; this initWithRequest:request delegate:delegate startImmediately:start_immediately]
}

- (())start {
    let should_start = {
        let host_object = env.objc.borrow::<NSURLConnectionHostObject>(this);
        !host_object.started && !host_object.cancelled
    };
    if should_start {
        schedule_failure(env, this);
    }
}

- (())cancel {
    let delegate = {
        let host_object = env.objc.borrow_mut::<NSURLConnectionHostObject>(this);
        host_object.cancelled = true;
        std::mem::replace(&mut host_object.delegate, nil)
    };
    release(env, delegate);
}

- (id)request {
    env.objc.borrow::<NSURLConnectionHostObject>(this).request
}

- (id)delegate {
    env.objc.borrow::<NSURLConnectionHostObject>(this).delegate
}

- (())setDelegate:(id)delegate {
    retain(env, delegate);
    let old_delegate = {
        let host_object = env.objc.borrow_mut::<NSURLConnectionHostObject>(this);
        std::mem::replace(&mut host_object.delegate, delegate)
    };
    release(env, old_delegate);
}

- (())pearDeliverFailure:(id)error { // NSError *
    let (delegate, cancelled) = {
        let mut host_object = env.objc.borrow_mut::<NSURLConnectionHostObject>(this);
        let cancelled = host_object.cancelled;
        // The connection finishes here, so its reference to the delegate ends.
        let delegate = if cancelled {
            nil
        } else {
            std::mem::replace(&mut host_object.delegate, nil)
        };
        (delegate, cancelled)
    };
    if cancelled {
        return;
    }
    // `delegate` owns the reference taken by the connection until it is
    // released at the end of this method.
    let delegate_owned = delegate;
    if delegate == nil {
        log_dbg!(
            "[(NSURLConnection *){:?}] request failed but there is no delegate to report to",
            this
        );
        return;
    }

    let sel: SEL = env
        .objc
        .register_host_selector("connection:didFailWithError:".to_string(), &mut env.mem);
    if msg![env; delegate respondsToSelector:sel] {
        () = msg![env; delegate connection:this didFailWithError:error];
    } else {
        log_dbg!(
            "[(NSURLConnection *){:?}] delegate {:?} does not implement connection:didFailWithError:, replying with connectionDidFinishLoading instead",
            this,
            delegate,
        );
        let sel: SEL = env
            .objc
            .register_host_selector("connectionDidFinishLoading:".to_string(), &mut env.mem);
        if msg![env; delegate respondsToSelector:sel] {
            () = msg![env; delegate connectionDidFinishLoading:this];
        }
    }

    release(env, delegate_owned);
}

- (())dealloc {
    log_dbg!("[(NSURLConnection*){:?} dealloc]", this);
    let &NSURLConnectionHostObject {
        request, delegate, ..
    } = env.objc.borrow(this);
    release(env, request);
    release(env, delegate);
    env.objc.dealloc_object(this, &mut env.mem)
}

@end

};

/// Queues the failure callback for the given connection on the main run loop.
///
/// Real connections never call their delegate synchronously from the
/// initializer, so the callback is delivered on a later run loop iteration.
fn schedule_failure(env: &mut Environment, connection: id) {
    env.objc
        .borrow_mut::<NSURLConnectionHostObject>(connection)
        .started = true;

    let run_loop: id = msg_class![env; NSRunLoop mainRunLoop];
    let sel = env
        .objc
        .register_host_selector(DELIVER_FAILURE_SELECTOR.to_string(), &mut env.mem);
    let error = make_not_connected_error(env);
    add_perform_request(env, run_loop, connection, sel, error, Some(0.0), false);
}

fn make_not_connected_error(env: &mut Environment) -> id {
    let domain = ns_string::get_static_str(env, NSURLErrorDomain);
    let error: id = msg_class![env; NSError alloc];
    // TODO: fill userInfo
    let error: id =
        msg![env; error initWithDomain:domain code:NSURLErrorNotConnectedToInternet userInfo:nil];
    autorelease(env, error)
}

fn url_string_from_request(env: &mut Environment, request: id) -> Cow<'static, str> {
    if request == nil {
        Cow::from("(null)")
    } else {
        let url = msg![env; request URL];
        let ns_string = msg![env; url absoluteString];
        ns_string::to_rust_string(env, ns_string)
    }
}
