use crate::ffi::failure;
use shinki_emu_abi::{Bytes, Error, Header, INVALID_ARGUMENT, INVALID_STATE, OK, PANICKED, Status};
use std::{ffi::c_void, sync::OnceLock};

const ID: &[u8] = b"shinki.emu.android_context";
const VERSION: u32 = 1;
const ERROR_CODE: &str = "SHINKI_EMU_ARTEMIS_ANDROID_CONTEXT";
static CONTEXT: OnceLock<(usize, usize)> = OnceLock::new();

// Match shinki_emu_abi::AndroidContextV1. This standalone adapter's locked ABI
// revision predates the additive table; its Header, Error and Status are ABI v3.
#[repr(C)]
struct AndroidContextV1 {
    header: Header,
    initialize: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut Error) -> Status>,
}

static TABLE: AndroidContextV1 = AndroidContextV1 {
    header: Header::of::<AndroidContextV1>(),
    initialize: Some(initialize),
};

pub(super) unsafe fn extension(id: Bytes, version: u32) -> Option<*const c_void> {
    (version == VERSION && unsafe { id.as_slice(ID.len()) } == Ok(ID))
        .then(|| (&raw const TABLE).cast())
}

pub(super) fn initialized() -> bool {
    CONTEXT.get().is_some()
}

unsafe extern "C" fn initialize(
    java_vm: *mut c_void,
    application_context: *mut c_void,
    error: *mut Error,
) -> Status {
    if java_vm.is_null() || application_context.is_null() {
        return failure(
            error,
            INVALID_ARGUMENT,
            ERROR_CODE,
            "JavaVM and Application global reference are required",
        );
    }
    let binding = (java_vm as usize, application_context as usize);
    match std::panic::catch_unwind(|| {
        *CONTEXT.get_or_init(|| {
            unsafe { ndk_context::initialize_android_context(java_vm, application_context) };
            binding
        })
    }) {
        Ok(existing) if existing == binding => {
            if !error.is_null() {
                unsafe { *error = Error::default() };
            }
            OK
        }
        Ok(_) => failure(
            error,
            INVALID_STATE,
            ERROR_CODE,
            "Android context is already bound to different process references",
        ),
        Err(_) => failure(error, PANICKED, ERROR_CODE, "Android context setup panicked"),
    }
}
