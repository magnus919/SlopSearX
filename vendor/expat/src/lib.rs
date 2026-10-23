//! Audited-candidate native boundary. C parser handles never leave parse().
//! Only source-needed direct Atom fields are retained; full XML is validated.
use std::{
    ffi::{CStr, c_char, c_int, c_void},
    panic::{AssertUnwindSafe, catch_unwind},
};
const ATOM: &str = "http://www.w3.org/2005/Atom}";
const ARXIV: &str = "http://arxiv.org/schemas/atom}";
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Entry {
    pub fields: [Option<String>; 8],
    pub authors: Vec<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Syntax,
    NativeMemory,
    OutputMemory,
    Callback,
    Configuration,
}
#[derive(Debug)]
pub struct Parsed {
    pub entries: Vec<Entry>,
    pub tracked_peak: usize,
    pub retained_capacity: usize,
}
unsafe extern "C" {
    fn ssx_worker_prepare() -> c_int;
    fn ssx_retain_charge(n: usize) -> c_int;
    fn ssx_retain_release(n: usize);
    fn ssx_expat_parse(
        input: *const c_char,
        length: usize,
        memory_limit: usize,
        user: *mut c_void,
        start: unsafe extern "C" fn(*mut c_void, *const c_char) -> c_int,
        end: unsafe extern "C" fn(*mut c_void) -> c_int,
        text: unsafe extern "C" fn(*mut c_void, *const c_char, usize) -> c_int,
        peak: *mut usize,
    ) -> c_int;
}
#[derive(Clone, Copy)]
struct Capture {
    depth: usize,
    field: usize,
    author: bool,
    accepts: bool,
}
struct State {
    depth: usize,
    current: Option<Entry>,
    author: bool,
    capture: Option<Capture>,
    entries: Vec<Entry>,
    memory: usize,
    error: Option<Error>,
}
impl State {
    fn reserve_entries(&mut self) -> Result<(), Error> {
        if self.entries.len() == self.entries.capacity() {
            let old = self.entries.capacity();
            let new = old
                .checked_mul(2)
                .and_then(|n| n.checked_add(4))
                .ok_or(Error::OutputMemory)?;
            self.charge(
                new.checked_mul(std::mem::size_of::<Entry>())
                    .ok_or(Error::OutputMemory)?,
            )?;
            self.entries
                .try_reserve_exact(new - self.entries.len())
                .map_err(|_| Error::OutputMemory)?;
            self.release(old * std::mem::size_of::<Entry>());
            if self.entries.capacity() > new {
                self.charge((self.entries.capacity() - new) * std::mem::size_of::<Entry>())?
            }
        }
        Ok(())
    }
    fn charge(&mut self, n: usize) -> Result<(), Error> {
        let next = self.memory.checked_add(n).ok_or(Error::OutputMemory)?;
        if unsafe { ssx_retain_charge(n) } == 0 {
            return Err(Error::OutputMemory);
        }
        self.memory = next;
        Ok(())
    }
    fn release(&mut self, n: usize) {
        unsafe { ssx_retain_release(n) };
        self.memory -= n;
    }
    fn start(&mut self, name: &str) -> Result<(), Error> {
        self.depth = self.depth.checked_add(1).ok_or(Error::OutputMemory)?;
        if let Some(c) = &mut self.capture {
            if self.depth > c.depth {
                c.accepts = false
            }
        }
        if self.depth == 2 && name.strip_prefix(ATOM) == Some("entry") {
            self.current = Some(Entry::default());
            self.capture = None;
            self.author = false;
            return Ok(());
        }
        if self.current.is_none() {
            return Ok(());
        }
        if self.depth == 3 {
            self.author = name.strip_prefix(ATOM) == Some("author");
            let kind = if let Some(local) = name.strip_prefix(ATOM) {
                match local {
                    "title" => Some(0),
                    "summary" => Some(1),
                    "id" => Some(2),
                    "published" => Some(3),
                    "updated" => Some(7),
                    _ => None,
                }
            } else if let Some(local) = name.strip_prefix(ARXIV) {
                match local {
                    "doi" => Some(4),
                    "journal_ref" => Some(5),
                    "comment" => Some(6),
                    _ => None,
                }
            } else {
                None
            };
            if let Some(kind) = kind {
                let entry = self.current.as_mut().unwrap();
                if entry.fields[kind].is_none() {
                    entry.fields[kind] = Some(String::new());
                    self.capture = Some(Capture {
                        depth: 3,
                        field: kind,
                        author: false,
                        accepts: true,
                    });
                }
            }
        } else if self.depth == 4 && self.author && name.strip_prefix(ATOM) == Some("name") {
            let old = self.current.as_ref().unwrap().authors.capacity();
            let len = self.current.as_ref().unwrap().authors.len();
            if len == old {
                let new = old
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(4))
                    .ok_or(Error::OutputMemory)?;
                self.charge(
                    new.checked_mul(std::mem::size_of::<String>())
                        .ok_or(Error::OutputMemory)?,
                )?;
                self.current
                    .as_mut()
                    .unwrap()
                    .authors
                    .try_reserve_exact(new - len)
                    .map_err(|_| Error::OutputMemory)?;
                self.release(old * std::mem::size_of::<String>());
                let actual = self.current.as_ref().unwrap().authors.capacity();
                if actual > new {
                    self.charge((actual - new) * std::mem::size_of::<String>())?
                }
            }
            let e = self.current.as_mut().unwrap();
            e.authors.push(String::new());
            self.capture = Some(Capture {
                depth: 4,
                field: e.authors.len() - 1,
                author: true,
                accepts: true,
            });
        }
        Ok(())
    }
    fn end(&mut self) -> Result<(), Error> {
        if self.capture.is_some_and(|c| c.depth == self.depth) {
            self.capture = None
        }
        if self.depth == 3 {
            self.author = false
        }
        if self.depth == 2 && self.current.is_some() {
            self.reserve_entries()?;
            self.entries.push(self.current.take().unwrap())
        }
        self.depth = self.depth.checked_sub(1).ok_or(Error::Callback)?;
        Ok(())
    }
    fn text(&mut self, text: &str) -> Result<(), Error> {
        let Some(c) = self.capture.filter(|c| c.accepts && c.depth == self.depth) else {
            return Ok(());
        };
        let e = self.current.as_ref().ok_or(Error::Callback)?;
        let s = if c.author {
            &e.authors[c.field]
        } else {
            e.fields[c.field].as_ref().ok_or(Error::Callback)?
        };
        let old = s.capacity();
        let needed = s.len().checked_add(text.len()).ok_or(Error::OutputMemory)?;
        if needed > old {
            let new = old.checked_mul(2).unwrap_or(needed).max(needed);
            self.charge(new)?;
            let e = self.current.as_mut().unwrap();
            let s = if c.author {
                &mut e.authors[c.field]
            } else {
                e.fields[c.field].as_mut().unwrap()
            };
            s.try_reserve_exact(new - s.len())
                .map_err(|_| Error::OutputMemory)?;
            let actual = s.capacity();
            self.release(old);
            if actual > new {
                self.charge(actual - new)?
            }
        }
        let e = self.current.as_mut().unwrap();
        let s = if c.author {
            &mut e.authors[c.field]
        } else {
            e.fields[c.field].as_mut().unwrap()
        };
        s.push_str(text);
        Ok(())
    }
}
unsafe fn callback(user: *mut c_void, f: impl FnOnce(&mut State) -> Result<(), Error>) -> c_int {
    // Expat is synchronous and single-threaded, with no callback re-entry API.
    let state = unsafe { &mut *user.cast::<State>() };
    if let Some(e) = state.error {
        return i32::from(e == Error::OutputMemory);
    }
    match catch_unwind(AssertUnwindSafe(|| f(state))) {
        Ok(Ok(())) => 1,
        Ok(Err(e)) => {
            state.error = Some(e);
            if e == Error::OutputMemory {
                state.current = None;
                state.entries = Vec::new();
                state.capture = None;
                state.release(state.memory);
                1
            } else {
                0
            }
        }
        Err(_) => {
            state.error = Some(Error::Callback);
            0
        }
    }
}
unsafe extern "C" fn start(user: *mut c_void, name: *const c_char) -> c_int {
    unsafe {
        callback(user, |s| {
            let name = CStr::from_ptr(name).to_str().map_err(|_| Error::Callback)?;
            s.start(name)
        })
    }
}
unsafe extern "C" fn end(user: *mut c_void) -> c_int {
    unsafe { callback(user, State::end) }
}
unsafe extern "C" fn text(user: *mut c_void, data: *const c_char, n: usize) -> c_int {
    unsafe {
        callback(user, |s| {
            if n == 0 {
                return Ok(());
            }
            let bytes = std::slice::from_raw_parts(data.cast::<u8>(), n);
            s.text(std::str::from_utf8(bytes).map_err(|_| Error::Callback)?)
        })
    }
}
/// One live requested-capacity quota covers native parser and retained fields,
/// including old+new reservation overlap. OS limits contain untracked allocator
/// overhead; this ledger alone is not a process RSS claim.
pub fn parse(input: &str, memory_limit: usize) -> Result<Parsed, Error> {
    let mut state = State {
        depth: 0,
        current: None,
        author: false,
        capture: None,
        entries: Vec::new(),
        memory: 0,
        error: None,
    };
    let mut peak = 0;
    let result = unsafe {
        ssx_expat_parse(
            input.as_ptr().cast(),
            input.len(),
            memory_limit,
            (&mut state as *mut State).cast(),
            start,
            end,
            text,
            &mut peak,
        )
    };
    if result == 2 {
        return Err(Error::NativeMemory);
    }
    if result == 1 {
        return Err(Error::Syntax);
    }
    if let Some(e) = state.error {
        return Err(e);
    }
    match result {
        0 => Ok(Parsed {
            entries: state.entries,
            tracked_peak: peak,
            retained_capacity: state.memory,
        }),
        1 => Err(Error::Syntax),
        2 => Err(Error::NativeMemory),
        3 => Err(Error::Callback),
        _ => Err(Error::Configuration),
    }
}

/// Helper-only post-exec setup; permanently closes inherited descriptors and
/// lowers hard limits. Must run before input, parser or worker threads exist.
pub fn prepare_worker() -> Result<(), Error> {
    if unsafe { ssx_worker_prepare() } == 0 {
        Ok(())
    } else {
        Err(Error::Configuration)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    unsafe extern "C" {
        fn ssx_allocator_selftest() -> c_int;
    }
    #[test]
    fn checked_native_allocator_overflow_realloc_failure_and_free() {
        assert_eq!(unsafe { ssx_allocator_selftest() }, 0);
    }
    #[test]
    fn parser_failure_and_callback_refusal_free_all_native_allocations() {
        for limit in [1, 32, 64, 128, 512, 1024, 4096, 65536] {
            let result = parse(
                "<x xmlns='http://www.w3.org/2005/Atom'><entry><title>data</title></entry></x>",
                limit,
            );
            assert!(
                !matches!(result, Err(Error::Configuration)),
                "native leak/config at {limit}"
            );
        }
        let xml = format!(
            "<x xmlns='http://www.w3.org/2005/Atom'><entry><title>{}</title></entry></x>",
            "x".repeat(300000)
        );
        assert!(matches!(parse(&xml, 512 * 1024), Err(Error::OutputMemory)));
    }
}
