//! Deliver a URL to one verified running Desktop process, never a bundle-wide
//! protocol handler that could select a different saved account.
use crate::profiles::Result;
use std::ffi::{c_char, c_void};

#[link(name = "Foundation", kind = "framework")]
#[link(name = "AppKit", kind = "framework")]
extern "C" {}
#[link(name = "objc")]
extern "C" {
    fn objc_getClass(name: *const c_char) -> *mut c_void;
    fn sel_registerName(name: *const c_char) -> *mut c_void;
    fn objc_msgSend();
}

struct Pool(*mut c_void);
impl Drop for Pool {
    fn drop(&mut self) {
        unsafe {
            let send: unsafe extern "C" fn(*mut c_void, *mut c_void) =
                std::mem::transmute(objc_msgSend as *const ());
            send(self.0, sel_registerName(c"drain".as_ptr()));
        }
    }
}
unsafe fn send_id(receiver: *mut c_void, selector: *const c_char) -> *mut c_void {
    let send: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void =
        std::mem::transmute(objc_msgSend as *const ());
    send(receiver, sel_registerName(selector))
}

/// Caller must verify the executable and exact profile belonging to this PID.
pub fn open(pid: u32, thread: Option<&str>) -> Result<()> {
    let pid = i32::try_from(pid).map_err(|_| "Invalid Codex process identity")?;
    // SAFETY: These Foundation/AppKit selectors have the declared signatures.
    // Every object remains alive inside this thread's autorelease pool. The
    // only payload is a validated public thread UUID, never auth or chat text.
    unsafe {
        let pool = send_id(
            objc_getClass(c"NSAutoreleasePool".as_ptr()),
            c"alloc".as_ptr(),
        );
        let _pool = Pool(send_id(pool, c"init".as_ptr()));
        let descriptor = objc_getClass(c"NSAppleEventDescriptor".as_ptr());
        if descriptor.is_null() {
            return Err("macOS URL routing is unavailable".into());
        }
        let address: unsafe extern "C" fn(*mut c_void, *mut c_void, i32) -> *mut c_void =
            std::mem::transmute(objc_msgSend as *const ());
        if let Some(thread) = thread {
            let target = address(
                descriptor,
                sel_registerName(c"descriptorWithProcessIdentifier:".as_ptr()),
                pid,
            );
            let uri = format!("codex://threads/{thread}");
            let string = send_id(objc_getClass(c"NSString".as_ptr()), c"alloc".as_ptr());
            let initialize: unsafe extern "C" fn(
                *mut c_void,
                *mut c_void,
                *const u8,
                usize,
                usize,
            ) -> *mut c_void = std::mem::transmute(objc_msgSend as *const ());
            let string = initialize(
                string,
                sel_registerName(c"initWithBytes:length:encoding:".as_ptr()),
                uri.as_ptr(),
                uri.len(),
                4,
            );
            let string = send_id(string, c"autorelease".as_ptr());
            let parameter: unsafe extern "C" fn(
                *mut c_void,
                *mut c_void,
                *mut c_void,
            ) -> *mut c_void = std::mem::transmute(objc_msgSend as *const ());
            let value = parameter(
                descriptor,
                sel_registerName(c"descriptorWithString:".as_ptr()),
                string,
            );
            let create: unsafe extern "C" fn(
                *mut c_void,
                *mut c_void,
                u32,
                u32,
                *mut c_void,
                i16,
                i32,
            ) -> *mut c_void = std::mem::transmute(objc_msgSend as *const ());
            let gurl = u32::from_be_bytes(*b"GURL");
            let event = create(
                descriptor,
                sel_registerName(
                    c"appleEventWithEventClass:eventID:targetDescriptor:returnID:transactionID:"
                        .as_ptr(),
                ),
                gurl,
                gurl,
                target,
                -1,
                0,
            );
            if event.is_null() || value.is_null() || target.is_null() {
                return Err("Could not prepare the Mac chat URL".into());
            }
            let set: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, u32) =
                std::mem::transmute(objc_msgSend as *const ());
            set(
                event,
                sel_registerName(c"setParamDescriptor:forKeyword:".as_ptr()),
                value,
                u32::from_be_bytes(*b"----"),
            );
            let send: unsafe extern "C" fn(
                *mut c_void,
                *mut c_void,
                usize,
                f64,
                *mut *mut c_void,
            ) -> *mut c_void = std::mem::transmute(objc_msgSend as *const ());
            let mut error = std::ptr::null_mut();
            // WaitForReply + CanSwitchLayer. Bounded native delivery lets the
            // HUD retain its attention badge if macOS rejects the operation.
            let reply = send(
                event,
                sel_registerName(c"sendEventWithOptions:timeout:error:".as_ptr()),
                3 | 64,
                10.0,
                &mut error,
            );
            if reply.is_null() {
                let code: unsafe extern "C" fn(*mut c_void, *mut c_void) -> isize =
                    std::mem::transmute(objc_msgSend as *const ());
                return Err(format!(
                    "macOS could not open this chat (URL event {})",
                    code(error, sel_registerName(c"code".as_ptr()))
                ));
            }
            let field: unsafe extern "C" fn(*mut c_void, *mut c_void, u32) -> *mut c_void =
                std::mem::transmute(objc_msgSend as *const ());
            let failure = field(
                reply,
                sel_registerName(c"paramDescriptorForKeyword:".as_ptr()),
                u32::from_be_bytes(*b"errn"),
            );
            let value: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32 =
                std::mem::transmute(objc_msgSend as *const ());
            let failure = value(failure, sel_registerName(c"int32Value".as_ptr()));
            if failure != 0 {
                return Err(format!("Codex rejected the Mac chat URL (event {failure})"));
            }
        }
        let application = address(
            objc_getClass(c"NSRunningApplication".as_ptr()),
            sel_registerName(c"runningApplicationWithProcessIdentifier:".as_ptr()),
            pid,
        );
        let activate: unsafe extern "C" fn(*mut c_void, *mut c_void, usize) -> bool =
            std::mem::transmute(objc_msgSend as *const ());
        if application.is_null()
            || !activate(
                application,
                sel_registerName(c"activateWithOptions:".as_ptr()),
                2,
            )
        {
            return Err("Could not focus the account's Codex window".into());
        }
    }
    Ok(())
}
