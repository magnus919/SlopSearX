//! Native replacement for the temporary Python Expat qualification driver.
use crate::{Result, command};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn bounded(root: &Path, program: &Path, args: &[&str], records: &mut Vec<Value>) -> Result<Output> {
    let mut child = Command::new(program)
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
            state => {
                let _ = child.kill();
                let reaped = child.wait().is_ok();
                records.push(json!({"native_resource_timeout_or_wait_error":format!("{state:?}"),"reaped":reaped}));
                return Err("A06 resource subprocess deadline/wait failure".into());
            }
        }
    }
    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    records.push(json!({"program":program,"args":args,"exit_code":output.status.code(),"stdout":String::from_utf8_lossy(&output.stdout),"stderr":String::from_utf8_lossy(&output.stderr)}));
    Ok(output)
}
fn exact_exit(actual: Option<i32>, expected: i32) -> Result<()> {
    if actual != Some(expected) {
        return Err(format!(
            "A06 expected terminated exit{expected}, got{actual:?}"
        ));
    }
    Ok(())
}
fn terminated(
    root: &Path,
    program: &Path,
    args: &[&str],
    expected: i32,
    records: &mut Vec<Value>,
) -> Result<String> {
    let output = bounded(root, program, args, records)?;
    exact_exit(output.status.code(), expected)?;
    String::from_utf8(output.stdout).map_err(|e| e.to_string())
}
fn memory_contract(output: &str, linux: bool) -> Result<()> {
    if linux {
        if !output.starts_with("GNU_AS=1073741824 ") {
            return Err("A06 GNU absolute address-space qualification missing".into());
        }
    } else if output.trim() != "developer-platform: no absolute RSS qualification" {
        return Err("A06 developer-platform resource classification mismatch".into());
    }
    Ok(())
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let built = command(
        root,
        "cargo",
        &[
            "build",
            "--offline",
            "--locked",
            "-p",
            "ssx-source-xml",
            "--message-format=json",
        ],
        records,
    )?;
    let mut out = None;
    for line in built.lines() {
        let row: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
        if row["reason"] == "build-script-executed"
            && row["package_id"]
                .as_str()
                .is_some_and(|s| s.contains("ssx-source-xml"))
        {
            out = row["out_dir"].as_str().map(PathBuf::from);
        }
    }
    let out = out.ok_or("A06 missing source XML build-script receipt")?;
    let id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let temp =
        Scratch(std::env::temp_dir().join(format!("ssx-a06-native-{}-{id}", std::process::id())));
    fs::create_dir(&temp.0).map_err(|e| e.to_string())?;
    let probe = temp.0.join("resource-probe");
    let launch = temp.0.join("resource-launch");
    let hang = temp.0.join("resource-hang");
    let source = root.join("vendor/expat/tests/resource_probe.c");
    let launch_source = root.join("vendor/expat/tests/resource_launch.c");
    let hang_source = root.join("vendor/expat/tests/resource_hang.c");
    let wrapper = out.join("libssx_expat_wrapper.a");
    let expat = out.join("native-build/libexpat.a");
    let text = |p: &Path| {
        p.to_str()
            .map(str::to_owned)
            .ok_or_else(|| "A06 native path is not UTF8".to_owned())
    };
    let source = text(&source)?;
    let launch_source = text(&launch_source)?;
    let wrapper = text(&wrapper)?;
    let expat = text(&expat)?;
    let probe_text = text(&probe)?;
    let launch_text = text(&launch)?;
    let hang_text = text(&hang)?;
    let hang_source = text(&hang_source)?;
    command(
        root,
        "cc",
        &[
            "-std=c11",
            "-O2",
            "-Wall",
            "-Wextra",
            "-Werror",
            &source,
            &wrapper,
            &expat,
            "-o",
            &probe_text,
        ],
        records,
    )?;
    command(
        root,
        "cc",
        &[
            "-std=c11",
            "-O2",
            "-Wall",
            "-Wextra",
            "-Werror",
            &launch_source,
            "-o",
            &launch_text,
        ],
        records,
    )?;
    let sentinel = text(&temp.0.join("sentinel"))?;
    command(
        root,
        "cc",
        &[
            "-std=c11",
            "-O2",
            "-Wall",
            "-Wextra",
            "-Werror",
            &hang_source,
            "-o",
            &hang_text,
        ],
        records,
    )?;
    let audit = terminated(root, &launch, &[&probe_text, &sentinel], 0, records)?;
    if audit.as_bytes() != b"stdio-ok\n" {
        return Err("A06 descriptor/stdin/stdout audit mismatch".into());
    }
    // Negative controls exercise the unchanged original probe's descriptor checks.
    terminated(root, &probe, &["-1", "-1"], 3, records)?;
    terminated(root, &probe, &[], 2, records)?;
    let hang_sentinel = text(&temp.0.join("hang-sentinel"))?;
    let cleanup = terminated(
        root,
        &launch,
        &[&hang_text, &hang_sentinel, "50", "--await-started"],
        47,
        records,
    )?;
    let cleanup: Value = serde_json::from_str(&cleanup).map_err(|e| e.to_string())?;
    let pid = fs::read_to_string(&hang_sentinel)
        .map_err(|e| e.to_string())?
        .trim()
        .parse::<u64>()
        .map_err(|e| e.to_string())?;
    if pid == 0
        || cleanup["owned_child"] != pid
        || cleanup["reaped"] != true
        || cleanup["signal"] != 9
        || cleanup["reason"] != "deadline"
        || cleanup["startup_acknowledged"] != true
        || cleanup["startup_elapsed_ms"]
            .as_u64()
            .is_none_or(|ms| ms >= 5000)
    {
        return Err("A06 hanging child was not explicitly killed and reaped".into());
    }
    records.push(json!({"A06_actual_hanging_child":cleanup,"sentinel_acknowledged_pid":pid}));
    let memory = terminated(root, &probe, &["--memory"], 0, records)?;
    memory_contract(&memory, cfg!(target_os = "linux"))?;
    records.push(json!({"A06_native_resource":{"descriptor_audit":"passed","inherited_file":"closed","inherited_AF_UNIX_socket":"closed-peer-EOF","stdio":"preserved","NOFILE":3,"memory":memory,"platform":std::env::consts::OS,"temporary_python":"not executed","scope":"same pinned native probe, not backend/global RSS or Darwin absolute limit proof"}}));
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn negative_controls_require_expected_terminated_exit_not_any_error() {
        exact_exit(Some(3), 3).unwrap();
        exact_exit(Some(2), 2).unwrap();
        for code in [None, Some(0), Some(1), Some(2), Some(127)] {
            assert!(exact_exit(code, 3).is_err());
        }
        let mut records = Vec::new();
        assert!(
            terminated(
                Path::new("."),
                Path::new("/nonexistent-a06-native-probe"),
                &[],
                3,
                &mut records
            )
            .is_err()
        );
    }
    #[test]
    fn absent_or_wrong_platform_memory_evidence_fails_closed() {
        assert!(memory_contract("", true).is_err());
        assert!(
            memory_contract("developer-platform: no absolute RSS qualification\n", true).is_err()
        );
        assert!(memory_contract("GNU_AS=1 allocated_blocks=1\n", true).is_err());
        memory_contract(
            "GNU_AS=1073741824 allocated_blocks=31 peak_rss_kib=1000\n",
            true,
        )
        .unwrap();
        memory_contract("developer-platform: no absolute RSS qualification\n", false).unwrap();
        assert!(memory_contract("GNU_AS=1073741824 ", false).is_err());
    }
}
