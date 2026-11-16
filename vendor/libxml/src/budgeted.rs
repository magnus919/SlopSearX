//! Dedicated-child allocation hooks. Experimental, default off, never restored.
//!
//! Install before any other libxml call. The root and callbacks live until process
//! exit; dropping the token does not restore hooks or release global C storage.
//! A successful parse is not publishable after `ChildAllocator::failed()` becomes
//! true, even when libxml returned a partial non-null document. This module is
//! not yet connected to the adapter parser or an immutable document facade.
#![deny(unsafe_op_in_unsafe_fn)]
use crate::bindings;
use ssx_owned_budget::{Budget, Charge};
use std::{
  alloc::Layout,
  ffi::{c_char, c_void},
  marker::PhantomData,
  mem::{ManuallyDrop, align_of},
  ptr,
  rc::Rc,
  sync::{
    OnceLock,
    atomic::{AtomicBool, AtomicUsize, Ordering},
  },
};
static ROOT: OnceLock<Budget> = OnceLock::new();
static OFFSET: OnceLock<usize> = OnceLock::new();
static FAILED: AtomicBool = AtomicBool::new(false);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static FREES: AtomicUsize = AtomicUsize::new(0);
#[repr(C)]
struct Header {
  length: usize,
  charge: ManuallyDrop<Charge>,
}
/// Installation failure. Hooks, once installed, are never restored.
#[derive(Debug, PartialEq, Eq)]
pub enum InstallError {
  /// A legacy entry point initialized libxml first, or installation ran before.
  AlreadyInitialized,
  /// The initialization gate was poisoned; installation is refused.
  Poisoned,
  /// Host alignment/layout cannot represent a conforming C allocation header.
  Alignment,
  /// libxml refused its complete callback table.
  CallbackTable,
  /// Global parser initialization exhausted the supplied root.
  Resource,
}
/// Non-transferable startup token for one dedicated child process.
///
/// There is deliberately no reset/restore API. The static root survives this
/// value, every C pointer and libxml global/thread-local cleanup until exit.
pub struct ChildAllocator {
  _local: PhantomData<Rc<()>>,
}
impl ChildAllocator {
  /// Install one budgeted callback table before any wrapper initialization.
  /// Direct users of unsafe raw bindings must uphold the same first-call rule.
  pub fn install(root: Budget) -> Result<Self, InstallError> {
    let mut initialized = crate::PARSER_INITIALIZED
      .lock()
      .map_err(|_| InstallError::Poisoned)?;
    if *initialized || ROOT.get().is_some() {
      return Err(InstallError::AlreadyInitialized);
    }
    match root.snapshot() {
      Ok(state) if !state.failed => {}
      _ => return Err(InstallError::Resource),
    }
    let alignment = align_of::<libc::max_align_t>();
    let layout = Layout::new::<Header>()
      .align_to(alignment)
      .map_err(|_| InstallError::Alignment)?
      .pad_to_align();
    if align_of::<Header>() > alignment || layout.size() > isize::MAX as usize {
      return Err(InstallError::Alignment);
    }
    OFFSET
      .set(layout.size())
      .map_err(|_| InstallError::AlreadyInitialized)?;
    ROOT
      .set(root)
      .map_err(|_| InstallError::AlreadyInitialized)?;
    // SAFETY: serialized gate proves no safe wrapper initialized libxml. All
    // callbacks have exact generated signatures and static lifetime. Root/header
    // configuration is already published and never changed/restored thereafter.
    let result = unsafe {
      bindings::xmlMemSetup(
        Some(free),
        Some(allocate),
        Some(reallocate),
        Some(duplicate),
      )
    };
    *initialized = true;
    if result != 0 {
      fail();
      return Err(InstallError::CallbackTable);
    }
    // SAFETY: callbacks/root are installed before any initialization allocation.
    unsafe {
      bindings::xmlInitParser();
    }
    let installed = Self {
      _local: PhantomData,
    };
    if installed.failed() {
      return Err(InstallError::Resource);
    }
    Ok(installed)
  }
  pub(crate) fn root_budget(&self) -> Option<&'static Budget> {
    ROOT.get()
  }
  /// Sticky allocation refusal. Never reinterpret a partial C tree as success.
  pub fn failed(&self) -> bool {
    FAILED.load(Ordering::SeqCst)
      || ROOT.get().is_none_or(|root| match root.snapshot() {
        Ok(state) => state.failed,
        Err(_) => true,
      })
  }
  /// Counts actual callback allocations/frees, including parser globals.
  pub fn callback_counts(&self) -> (usize, usize) {
    (
      ALLOCATIONS.load(Ordering::SeqCst),
      FREES.load(Ordering::SeqCst),
    )
  }
}
fn fail() {
  FAILED.store(true, Ordering::SeqCst);
  if let Some(root) = ROOT.get() {
    root.fail_closed();
  }
}
unsafe extern "C" fn allocate(length: usize) -> *mut c_void {
  let Some(offset) = OFFSET.get().copied() else {
    fail();
    return ptr::null_mut();
  };
  let Some(total) = offset
    .checked_add(length)
    .filter(|n| *n <= isize::MAX as usize)
  else {
    fail();
    return ptr::null_mut();
  };
  let Some(root) = ROOT.get() else {
    fail();
    return ptr::null_mut();
  };
  let charge = match root.reserve(total) {
    Ok(c) => c,
    Err(_) => {
      fail();
      return ptr::null_mut();
    }
  };
  // SAFETY: libc malloc accepts the checked nonzero total. Its base satisfies
  // fundamental max_align_t alignment; checked padded header preserves it.
  let base = unsafe { libc::malloc(total) };
  if base.is_null() {
    drop(charge);
    fail();
    return ptr::null_mut();
  }
  if !(base as usize).is_multiple_of(align_of::<libc::max_align_t>()) {
    unsafe { libc::free(base) };
    drop(charge);
    fail();
    return ptr::null_mut();
  }
  // SAFETY: allocation contains aligned Header followed by length writable bytes.
  unsafe {
    ptr::write(
      base.cast::<Header>(),
      Header {
        length,
        charge: ManuallyDrop::new(charge),
      },
    );
  }
  ALLOCATIONS.fetch_add(1, Ordering::SeqCst);
  // A zero-size request still receives an owned nonnull pointer, freed normally.
  unsafe { base.cast::<u8>().add(offset).cast() }
}
unsafe extern "C" fn free(memory: *mut c_void) {
  if memory.is_null() {
    return;
  }
  let Some(offset) = OFFSET.get().copied() else {
    fail();
    return;
  };
  // SAFETY: C must pass only null or a pointer returned by this installed table.
  // The header is moved out before freeing storage; charge release follows free.
  let base = unsafe { memory.cast::<u8>().sub(offset) };
  let header = unsafe { ptr::read(base.cast::<Header>()) };
  unsafe { libc::free(base.cast()) };
  drop(ManuallyDrop::into_inner(header.charge));
  FREES.fetch_add(1, Ordering::SeqCst);
}
unsafe extern "C" fn reallocate(memory: *mut c_void, length: usize) -> *mut c_void {
  if memory.is_null() {
    return unsafe { allocate(length) };
  }
  let Some(offset) = OFFSET.get().copied() else {
    fail();
    return ptr::null_mut();
  };
  // SAFETY: original C allocation remains live through full new allocation and
  // copy. Failure leaves it untouched and charged. No libc realloc can conceal
  // an old/new overlap; zero length intentionally returns a new owned block.
  let old_length = unsafe { (*memory.cast::<u8>().sub(offset).cast::<Header>()).length };
  let next = unsafe { allocate(length) };
  if next.is_null() {
    return ptr::null_mut();
  }
  unsafe {
    ptr::copy_nonoverlapping(
      memory.cast::<u8>(),
      next.cast::<u8>(),
      old_length.min(length),
    );
    free(memory);
  }
  next
}
unsafe extern "C" fn duplicate(value: *const c_char) -> *mut c_char {
  if value.is_null() {
    return ptr::null_mut();
  }
  // SAFETY: libxml strdup ABI requires a readable NUL-terminated C string.
  let length = unsafe { libc::strlen(value) };
  let Some(total) = length.checked_add(1) else {
    fail();
    return ptr::null_mut();
  };
  let next = unsafe { allocate(total) }.cast::<c_char>();
  if !next.is_null() {
    unsafe {
      ptr::copy_nonoverlapping(value, next, total);
    }
  }
  next
}

// Only C pointers allocated under this process-lifetime table reach this helper.
pub(crate) unsafe fn release_owned(memory: *mut c_void) {
  unsafe {
    free(memory);
  }
}
