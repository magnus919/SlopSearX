//! Row-producing supervised worker. C allocations and flat-tree backing share
//! the child root; extractor/sanitizer transient ownership remains under audit.
use super::{
    MAX_POINTS,
    row_wire::{self, ExtendedRow, ImageMedia, Layout, Row},
};
use crate::{adapters::RowLimit, html_budget::MAX_SEARCH_BYTES};
use ssx_owned_budget::{Budget, OwnedVec};
use std::io::{Read, Write};

pub(super) const VERSION: u8 = 3;
pub(super) const HEADER: usize = 32;
pub(super) struct Request {
    pub(super) layout: Layout,
    pub(super) limit: RowLimit,
    pub(super) count: usize,
    pub(super) allowance: usize,
}
enum Results {
    A05(OwnedVec<super::child_rows::Row>),
    A10(crate::adapters::vulnerability_html::Rows),
}
impl Request {
    pub(super) fn encode(&self) -> Result<[u8; HEADER], &'static str> {
        if self.count > MAX_POINTS || self.allowance == 0 || self.allowance > MAX_SEARCH_BYTES {
            return Err("invalid HTML rows allowance/count");
        }
        if self.layout.is_a10()
            != matches!(
                self.limit,
                RowLimit::Slice(_) | RowLimit::SliceAll | RowLimit::InvalidSlice
            )
            || (matches!(
                self.layout,
                Layout::MitreTechnique | Layout::MitreGroup | Layout::MitreSoftware
            ) && !matches!(self.limit, RowLimit::SliceAll))
        {
            return Err("HTML rows layout/limit mismatch");
        }
        let layout = match self.layout {
            Layout::DdgHtml => 0,
            Layout::DdgLite => 1,
            Layout::DdgImage => 2,
            Layout::Google => 3,
            Layout::ExploitDb => 4,
            Layout::MitreTechnique => 5,
            Layout::MitreGroup => 6,
            Layout::MitreSoftware => 7,
        };
        let (tag, bits) = match self.limit {
            RowLimit::Number(number) => (0, number.to_bits()),
            RowLimit::Unlimited => (1, 0),
            RowLimit::Invalid(tag @ 0..=3) => (tag + 2, 0),
            RowLimit::Invalid(_) => return Err("invalid HTML rows limit"),
            RowLimit::Slice(n) => (6, u64::from_be_bytes(n.to_be_bytes())),
            RowLimit::SliceAll => (7, 0),
            RowLimit::InvalidSlice => (8, 0),
        };
        let mut bytes = [0; HEADER];
        bytes[..8].copy_from_slice(&[b'A', b'0', b'5', b'H', VERSION, 2, layout, tag]);
        bytes[8..16].copy_from_slice(&bits.to_be_bytes());
        bytes[16..24].copy_from_slice(&(self.count as u64).to_be_bytes());
        bytes[24..].copy_from_slice(&(self.allowance as u64).to_be_bytes());
        Ok(bytes)
    }
    fn decode(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() != HEADER || bytes[..6] != [b'A', b'0', b'5', b'H', VERSION, 2] {
            return Err("invalid HTML rows header");
        }
        let layout = match bytes[6] {
            0 => Layout::DdgHtml,
            1 => Layout::DdgLite,
            2 => Layout::DdgImage,
            3 => Layout::Google,
            4 => Layout::ExploitDb,
            5 => Layout::MitreTechnique,
            6 => Layout::MitreGroup,
            7 => Layout::MitreSoftware,
            _ => return Err("invalid HTML rows layout"),
        };
        let bits = u64::from_be_bytes(bytes[8..16].try_into().unwrap());
        let limit = match (bytes[7], bits) {
            (0, bits) => RowLimit::Number(f64::from_bits(bits)),
            (1, 0) => RowLimit::Unlimited,
            (tag @ 2..=5, 0) => RowLimit::Invalid(tag - 2),
            (6, bits) => RowLimit::Slice(i64::from_be_bytes(bits.to_be_bytes())),
            (7, 0) => RowLimit::SliceAll,
            (8, 0) => RowLimit::InvalidSlice,
            _ => return Err("invalid HTML rows limit"),
        };
        let request = Self {
            layout,
            limit,
            count: usize::try_from(u64::from_be_bytes(bytes[16..24].try_into().unwrap()))
                .map_err(|_| "HTML rows count overflow")?,
            allowance: usize::try_from(u64::from_be_bytes(bytes[24..].try_into().unwrap()))
                .map_err(|_| "HTML rows allowance overflow")?,
        };
        request.encode()?;
        Ok(request)
    }
}
pub(super) const ERRORS: [&str; 7] = [
    "Document is empty",
    "Unicode strings with encoding declaration are not supported. Please use bytes input or XML fragments without declaration.",
    "'>=' not supported between instances of 'int' and 'NoneType'",
    "'>=' not supported between instances of 'int' and 'str'",
    "'>=' not supported between instances of 'int' and 'list'",
    "'>=' not supported between instances of 'int' and 'dict'",
    "slice indices must be integers or None or have an __index__ method",
];
fn failure(output: &mut impl Write, tag: u8, code: u8) -> std::io::Result<()> {
    output.write_all(&7u64.to_be_bytes())?;
    output.write_all(&[b'A', b'0', b'5', b'E', 1, tag, code])?;
    output.flush()
}
pub(super) fn worker(input: &mut impl Read, output: &mut impl Write) -> Result<(), String> {
    let mut header = [0; HEADER];
    header[..5].copy_from_slice(b"A05H\x03");
    input
        .read_exact(&mut header[5..])
        .map_err(|error| error.to_string())?;
    let request = Request::decode(&header).map_err(str::to_owned)?;
    let budget = Budget::new(request.allowance).map_err(|error| error.to_string())?;
    // Hooks must be installed before the first C parser/global initialization.
    // This worker serves one request; root and hooks remain alive until process
    // exit, including all document/getter destruction paths.
    let allocator = match libxml::budgeted::ChildAllocator::install(budget.clone()) {
        Ok(allocator) => allocator,
        Err(_) => return failure(output, 1, 0).map_err(|error| error.to_string()),
    };
    let result = (|| -> Result<_, (u8, u8)> {
        let mut points = OwnedVec::with_capacity(&budget, request.count).map_err(|_| (1, 0))?;
        let mut scratch = [0; 4096];
        let mut remaining = request.count;
        while remaining != 0 {
            let count = remaining.min(scratch.len() / 4);
            input
                .read_exact(&mut scratch[..count * 4])
                .map_err(|_| (3, 0))?;
            for point in scratch[..count * 4].as_chunks::<4>().0 {
                let point = u32::from_be_bytes(*point);
                if point > 0x10ffff {
                    return Err((3, 0));
                }
                points.try_push(point).map_err(|_| (1, 0))?;
            }
            remaining -= count;
        }
        let mut trailing = [0];
        if input.read(&mut trailing).map_err(|_| (3, 0))? != 0 {
            return Err((3, 0));
        }
        let doc = super::charged_flatten::parse(points.as_slice(), &budget, &allocator).map_err(
            |error| match error {
                super::OperationFailure::Resource => (1, 0),
                super::OperationFailure::Parse(message) => ERRORS
                    .iter()
                    .position(|s| *s == message)
                    .map(|code| (2, code as u8))
                    .unwrap_or((3, 0)),
                _ => (3, 0),
            },
        )?;
        let results = if request.layout.is_a10() {
            crate::adapters::vulnerability_html::extract(
                &doc,
                points.as_slice(),
                request.layout,
                request.limit,
                &budget,
            )
            .map(Results::A10)
        } else {
            crate::adapters::html_extract_rows(&doc, request.layout, request.limit, &budget)
                .map(Results::A05)
        }
        .map_err(|error| match error {
            super::OperationFailure::Resource => (1, 0),
            super::OperationFailure::Parse(message) => ERRORS
                .iter()
                .position(|s| *s == message)
                .map(|code| (2, code as u8))
                .unwrap_or((3, 0)),
            _ => (3, 0),
        })?;
        drop(doc);
        drop(points);
        Ok(results)
    })();
    match result {
        Ok(Results::A10(results)) => {
            match crate::adapters::vulnerability_wire::size(request.layout, &results) {
                Ok(size) if size <= request.allowance => {}
                Ok(_) | Err(super::OperationFailure::Resource) => {
                    return failure(output, 1, 0).map_err(|error| error.to_string());
                }
                Err(_) => return failure(output, 3, 0).map_err(|error| error.to_string()),
            }
            crate::adapters::vulnerability_wire::write(request.layout, &results, output)
                .map_err(|error| error.to_string())
        }
        Ok(Results::A05(results)) => {
            let mut rows = match OwnedVec::with_capacity(&budget, results.len()) {
                Ok(rows) => rows,
                Err(_) => return failure(output, 1, 0).map_err(|error| error.to_string()),
            };
            for result in results.as_slice() {
                rows.try_push(ExtendedRow {
                    base: Row {
                        position: result.position,
                        query_title_fallback: request.layout == Layout::DdgImage
                            && result.title.is_empty(),
                        url: result.url.as_str(),
                        title: result.title.as_str(),
                        content: result.content.as_str(),
                        img_src: result.img_src.as_ref().map(|s| s.as_str()),
                    },
                    media: result.media.as_ref().map(|media| ImageMedia {
                        url: media.url.as_ref().map(|s| s.as_str()),
                        thumbnail: media.thumbnail.as_ref().map(|s| s.as_str()),
                        source: media.source.as_ref().map(|s| s.as_str()),
                    }),
                })
                .map_err(|error| error.to_string())?;
            }
            match row_wire::extended_size(request.layout, rows.as_slice()) {
                Ok(size) if size <= request.allowance => {}
                Ok(_) | Err(row_wire::Error::Size | row_wire::Error::Overflow) => {
                    return failure(output, 1, 0).map_err(|error| error.to_string());
                }
                Err(_) => return failure(output, 3, 0).map_err(|error| error.to_string()),
            }
            row_wire::write_extended(request.layout, rows.as_slice(), output)
                .map_err(|error| error.to_string())
        }
        Err((tag, code)) => {
            let mut discarded = 0usize;
            let mut scratch = [0; 4096];
            loop {
                let count = input
                    .read(&mut scratch)
                    .map_err(|error| error.to_string())?;
                if count == 0 {
                    break;
                }
                discarded = discarded
                    .checked_add(count)
                    .ok_or("HTML rows drain overflow")?;
                if discarded > MAX_POINTS * 4 {
                    return Err("HTML rows oversized trailing input".into());
                }
            }
            failure(output, tag, code).map_err(|error| error.to_string())
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closed_row_request_preserves_raw_limit_bits_and_rejects_malformed_fields() {
        for layout in [
            Layout::DdgHtml,
            Layout::DdgLite,
            Layout::DdgImage,
            Layout::Google,
        ] {
            for bits in [
                0,
                (-0.0f64).to_bits(),
                1.5f64.to_bits(),
                f64::INFINITY.to_bits(),
                f64::NEG_INFINITY.to_bits(),
                0x7ff8000000000012,
            ] {
                let request = Request {
                    layout,
                    limit: RowLimit::Number(f64::from_bits(bits)),
                    count: 3,
                    allowance: 1024,
                };
                let bytes = request.encode().unwrap();
                let parsed = Request::decode(&bytes).unwrap();
                assert_eq!(parsed.layout, layout);
                assert!(matches!(parsed.limit,RowLimit::Number(value) if value.to_bits()==bits));
                for length in 0..HEADER {
                    assert!(Request::decode(&bytes[..length]).is_err());
                }
            }
        }
        let request = Request {
            layout: Layout::Google,
            limit: RowLimit::Unlimited,
            count: 0,
            allowance: 1,
        };
        let bytes = request.encode().unwrap();
        for offset in [0, 4, 5, 6, 7] {
            let mut invalid = bytes;
            invalid[offset] = 255;
            assert!(Request::decode(&invalid).is_err());
        }
        for offset in [8, 24] {
            let mut invalid = bytes;
            invalid[offset..offset + 8].copy_from_slice(&u64::MAX.to_be_bytes());
            assert!(Request::decode(&invalid).is_err());
        }
        for tag in 2..=5 {
            let mut valid = bytes;
            valid[7] = tag;
            assert!(
                matches!(Request::decode(&valid).unwrap().limit,RowLimit::Invalid(value) if value==tag-2)
            );
        }
    }
    #[tokio::test]
    async fn actual_affine_tree_resource_refusal_and_next_worker_success() {
        use crate::process_pool::ProcessPool;
        use std::num::NonZeroUsize;
        let exe = std::env::current_exe().unwrap();
        let directory = exe.parent().unwrap();
        let directory = if directory.file_name().unwrap() == "deps" {
            directory.parent().unwrap()
        } else {
            directory
        };
        let pool = ProcessPool::new(
            directory.join("ssx-html-parser"),
            NonZeroUsize::new(1).unwrap(),
        )
        .unwrap();
        let raw = "<div>text</div>".repeat(20_000);
        let points: Vec<u32> = raw.chars().map(u32::from).collect();
        let payload: Vec<u8> = points.iter().flat_map(|p| p.to_be_bytes()).collect();
        for allowance in [2 * 1024 * 1024, 32 * 1024 * 1024] {
            let header = Request {
                layout: Layout::Google,
                limit: RowLimit::Unlimited,
                count: points.len(),
                allowance,
            }
            .encode()
            .unwrap();
            let reply = pool
                .exchange(
                    tokio::time::Instant::now() + std::time::Duration::from_secs(10),
                    header.to_vec(),
                    payload.clone(),
                    4096,
                )
                .await
                .unwrap();
            if allowance == 2 * 1024 * 1024 {
                assert_eq!(
                    reply,
                    [b'A', b'0', b'5', b'E', 1, 1, 0],
                    "resource refusal is terminal, never partial rows"
                );
            } else {
                let validated =
                    super::super::rows_borrowed::Rows::validate(&reply, Layout::Google, &|| Ok(()))
                        .await
                        .unwrap();
                assert_eq!(
                    validated.count(),
                    0,
                    "same hostile tree completes when quota permits; no provider rows"
                );
            }
        }
        // Explicit shutdown waits for the shared supervisor's verified reap;
        // this is a resource-outcome control, not a PID/CPU-phase timing proof.
        pool.shutdown().await.unwrap();
    }
    #[tokio::test]
    async fn hostile_sanitizer_workload_cancel_and_deadline_reap_owned_child() {
        use crate::{
            html::{OperationFailure, ParserPool},
            html_budget::{MAX_SEARCH_BYTES, Operation},
        };
        use std::{
            io::Read,
            num::NonZeroUsize,
            os::unix::fs::PermissionsExt,
            sync::{
                Arc,
                atomic::{AtomicU64, Ordering},
            },
        };
        // Shell fixture execs this exact lib-test binary; this branch executes
        // the actual row worker, native C tree and bounded shared sanitizer.
        if std::env::var_os("A05_SANITIZER_ENTRY_FILE").is_some() {
            let mut input = std::io::stdin().lock();
            let mut prefix = [0; 5];
            input.read_exact(&mut prefix).unwrap();
            assert_eq!(&prefix, b"A05H\x03");
            super::worker(&mut input, &mut std::io::sink()).unwrap();
            panic!("hostile worker unexpectedly completed before controlled termination");
        }
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        fn cpu_millis(pid: usize) -> Option<u64> {
            let output = std::process::Command::new("/bin/ps")
                .args(["-p", &pid.to_string(), "-o", "time="])
                .output()
                .ok()?;
            if !output.status.success() {
                return None;
            }
            let value = std::str::from_utf8(&output.stdout).ok()?.trim();
            let mut total = 0u64;
            for part in value.split(':') {
                let (whole, fraction) = part.split_once('.').unwrap_or((part, ""));
                let whole = whole.parse::<u64>().ok()?;
                let mut fraction_millis = 0u64;
                let mut place = 100;
                for byte in fraction.bytes().take(3) {
                    if !byte.is_ascii_digit() {
                        return None;
                    }
                    fraction_millis += u64::from(byte - b'0') * place;
                    place /= 10;
                }
                total = total
                    .checked_mul(60)?
                    .checked_add(whole.checked_mul(1000)?)?
                    .checked_add(fraction_millis)?;
            }
            Some(total)
        }
        let exe = std::env::current_exe().unwrap();
        let query = (0..12_000)
            .map(|n| format!("k{n}=first&k{n}=second"))
            .collect::<Vec<_>>()
            .join("&");
        let raw = format!(
            "<div class=tile><a href='https://source.test/'><img src='https://image.test/?{query}' alt='image'></a></div>"
        );
        let points: Vec<u32> = raw.chars().map(u32::from).collect();
        for deadline_mode in [false, true] {
            let directory = loop {
                let path = std::env::temp_dir().join(format!(
                    "a05-media-{}-{}",
                    std::process::id(),
                    SEQUENCE.fetch_add(1, Ordering::Relaxed)
                ));
                match std::fs::create_dir(&path) {
                    Ok(()) => break path,
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("private fixture: {error}"),
                }
            };
            let marker = directory.join("entered");
            let command = directory.join("worker");
            let quote =
                |s: &std::path::Path| format!("'{}'", s.to_str().unwrap().replace('\'', "'\\''"));
            // FD3 deliberately keeps the ProcessPool reply pipe alive while the
            // lib-test harness stdout is suppressed. Exec preserves the actual
            // PID; child exit closes FD3. No descendant manager is introduced.
            let script = format!(
                "#!/bin/sh\nexport A05_SANITIZER_ENTRY_FILE={}\nexec 3>&1\nexec {} --exact html::rows_worker::tests::hostile_sanitizer_workload_cancel_and_deadline_reap_owned_child --nocapture >/dev/null\n",
                quote(&marker),
                quote(&exe)
            );
            std::fs::write(&command, &script).unwrap();
            std::fs::set_permissions(&command, std::fs::Permissions::from_mode(0o700)).unwrap();
            let pool =
                Arc::new(ParserPool::new(command.clone(), NonZeroUsize::new(1).unwrap()).unwrap());
            let operation = Operation::start(
                MAX_SEARCH_BYTES,
                0,
                tokio::time::Instant::now() + std::time::Duration::from_secs(15),
            )
            .await
            .unwrap();
            let budget = operation.budget.0.clone();
            let weak = Arc::downgrade(&operation);
            let task_pool = pool.clone();
            let task_points = points.clone();
            let task = tokio::spawn(async move {
                task_pool
                    .project_rows_with_operation(
                        &task_points,
                        Layout::DdgImage,
                        RowLimit::Unlimited,
                        "",
                        operation,
                    )
                    .await
            });
            let observed = tokio::time::timeout(std::time::Duration::from_secs(8), async {
                loop {
                    if let Ok(text) = std::fs::read_to_string(&marker)
                        && text.ends_with('\n')
                    {
                        let values: Vec<_> =
                            text.split_whitespace().map(str::parse::<usize>).collect();
                        if let [Ok(pid), Ok(used)] = values.as_slice() {
                            break (*pid, *used);
                        }
                    }
                    if task.is_finished() {
                        break (0, 0);
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(2)).await;
                }
            })
            .await;
            let Ok((pid, child_used)) = observed else {
                task.abort();
                let _ = task.await;
                let cleanup = pool.shutdown().await;
                panic!(
                    "phase observation expired: {} cleanup={cleanup:?}",
                    directory.display()
                );
            };
            if pid == 0 || child_used == 0 || task.is_finished() {
                task.abort();
                let _ = task.await;
                let cleanup = pool.shutdown().await;
                panic!(
                    "phase failed pid={pid} child_used={child_used} cleanup={cleanup:?} fixture={}",
                    directory.display()
                );
            }
            // After this marker the synchronous child performs only the bounded
            // shared sanitizer; observe process CPU advancing before termination.
            // macOS reports fractional seconds, GNU ps may report whole seconds.
            let cpu_before = cpu_millis(pid);
            let cpu_after = tokio::time::timeout(std::time::Duration::from_secs(5), async {
                loop {
                    if let (Some(before), Some(after)) = (cpu_before, cpu_millis(pid))
                        && after > before
                    {
                        break Some(after);
                    }
                    if task.is_finished() {
                        break None;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            })
            .await
            .ok()
            .flatten();
            let parent_used = budget.snapshot().unwrap().used;
            let owner_live = weak.upgrade().is_some();
            let command_line = std::process::Command::new("/bin/ps")
                .args(["-p", &pid.to_string(), "-o", "command="])
                .output()
                .unwrap();
            let exact_process = command_line.status.success()
                && String::from_utf8_lossy(&command_line.stdout).contains(exe.to_str().unwrap());
            // This is observed sanitizer CALL entry with an active hostile
            // workload, not an injected hold or cooperative inner checkpoint.
            let outcome = if deadline_mode {
                tokio::time::pause();
                tokio::time::advance(std::time::Duration::from_secs(16)).await;
                tokio::time::resume();
                matches!(task.await, Ok(Err(OperationFailure::Deadline)))
            } else {
                task.abort();
                matches!(task.await, Err(error) if error.is_cancelled())
            };
            let cleanup =
                tokio::time::timeout(std::time::Duration::from_secs(5), pool.shutdown()).await;
            let dead = !std::process::Command::new("/bin/kill")
                .args(["-0", &pid.to_string()])
                .output()
                .unwrap()
                .status
                .success();
            let released = budget.snapshot().unwrap().used;
            println!(
                "A05_MEDIA_CHILD mode={} pid={pid} child_used={child_used} parent_used={parent_used} cpu_before={cpu_before:?} cpu_after={cpu_after:?} dead={dead} released={released} fixture={}",
                if deadline_mode { "deadline" } else { "cancel" },
                directory.display()
            );
            assert!(
                cpu_after.is_some(),
                "native sanitizer CPU did not advance after observed call entry"
            );
            assert!(outcome, "wrong terminal outcome");
            assert!(matches!(cleanup, Ok(Ok(()))), "cleanup {cleanup:?}");
            assert!(owner_live && parent_used > 0);
            assert!(exact_process, "fixture must exec actual tested PID");
            assert!(dead);
            assert_eq!(released, 0);
            assert!(weak.upgrade().is_none());
            // Receipt bytes survive successful fixture retirement in captured stdout.
            // On any earlier failure the controller retains files until child retirement.
            let entered = std::fs::read(&marker).unwrap();
            assert!(
                script.len() <= 8192 && entered.len() <= 128,
                "private fixture receipt bound"
            );
            println!("A05_MEDIA_FIXTURE_RECEIPT command={script:?} entered={entered:?}");
            std::fs::remove_file(&command).unwrap();
            std::fs::remove_file(&marker).unwrap();
            std::fs::remove_dir(&directory).unwrap();
            assert!(
                !directory.exists(),
                "successful owned fixture must be absent"
            );
        }
    }
}
