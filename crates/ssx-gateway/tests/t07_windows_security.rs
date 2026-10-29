//! Windows native boundaries; every case executes, with no privilege-based skip.
#![cfg(all(windows, feature = "windows-owner-test-fixtures"))]
mod platform_support;
use platform_support::{BUDGET, root, stage};
use ssx_windows_owner::{
    ConfidentialRead, FixtureProcessWitness, ImageRole, NativeChild, TrustedImage,
    build_file_digest, fixture_public_access,
};
use std::{io::Write, sync::Arc, time::Instant};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

#[test]
fn windows_package_lock_contenders_share_open_but_serialize_and_reject_identity_drift() {
    use std::os::windows::fs::OpenOptionsExt;
    let root = root("package-lock");
    let path = root.join(".gateway-package.lock");
    let first = ssx_windows_owner::package_lock(&path).unwrap();
    // FILE_SHARE_READ | FILE_SHARE_WRITE, without FILE_SHARE_DELETE. Opening
    // while the creator is alive deterministically rejects the old share=0.
    let second = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0x00000001 | 0x00000002)
        .open(&path)
        .unwrap();
    assert!(matches!(
        second.try_lock(),
        Err(std::fs::TryLockError::WouldBlock)
    ));
    assert!(
        std::fs::remove_file(&path).is_err(),
        "live lock cannot be deleted"
    );
    drop(first);
    second.try_lock().unwrap();
    drop(second);
    drop(ssx_windows_owner::package_lock(&path).unwrap());
    std::fs::write(&path, b"not an empty lock").unwrap();
    assert!(ssx_windows_owner::package_lock(&path).is_err());
    std::fs::write(&path, b"").unwrap();
    let alias = root.join("lock-alias");
    std::fs::hard_link(&path, &alias).unwrap();
    assert!(ssx_windows_owner::package_lock(&path).is_err());
    std::fs::remove_file(&alias).unwrap();
    fixture_public_access(&path, true).unwrap();
    assert!(ssx_windows_owner::package_lock(&path).is_err());
}

#[tokio::test]
async fn windows_token_storage_confidentiality_atomic_replacement_and_collision_ownership() {
    use ssx_mcp::gateway_client::{
        io_owner::IoOwner,
        token_storage::{FileTokenStorage, Slot},
    };
    let root = root("token");
    let path = root.join("oauth.json");
    let io = Arc::new(IoOwner::new());
    let cancel = tokio_util::sync::CancellationToken::new();
    let end = Instant::now() + BUDGET;
    let store = FileTokenStorage::load(path.clone(), io.clone(), end, &cancel)
        .await
        .unwrap();
    for bytes in [
        br#"{"access_token":"first-fixture"}"#.as_slice(),
        br#"{"access_token":"second-fixture"}"#,
    ] {
        store
            .set_slot_json(Slot::Tokens, bytes, end, &cancel)
            .await
            .unwrap();
        ssx_windows_owner::check_confidential_file(&path).unwrap();
        let reloaded = FileTokenStorage::load(path.clone(), io.clone(), end, &cancel)
            .await
            .unwrap();
        assert_eq!(
            reloaded
                .slot_json(Slot::Tokens, end, &cancel)
                .await
                .unwrap()
                .unwrap(),
            bytes
        );
    }
    let collision = root.join(".slopsearx-oauth-00000000000000000000000000000000.tmp");
    std::fs::write(&collision, b"not-owned").unwrap();
    assert!(ssx_windows_owner::write_confidential_file(&path, b"{}", [0; 16]).is_err());
    assert_eq!(std::fs::read(&collision).unwrap(), b"not-owned");
    // Public read is permitted for package integrity, denied for token storage.
    fixture_public_access(&path, false).unwrap();
    assert!(ssx_windows_owner::package_file_digest(&path).is_ok());
    assert!(
        FileTokenStorage::load(path.clone(), io.clone(), end, &cancel)
            .await
            .is_err()
    );
    assert!(ssx_windows_owner::write_confidential_file(&path, b"{}", [1; 16]).is_err());
    io.shutdown().await.unwrap();
}

#[test]
fn windows_token_reparse_hardlink_and_untrusted_writer_parent_are_rejected() {
    let root = root("token-paths");
    let target = root.join("target");
    ssx_windows_owner::create_private_directory(&target).unwrap();
    let path = target.join("oauth.json");
    ssx_windows_owner::write_confidential_file(&path, b"{}", [2; 16]).unwrap();
    assert!(matches!(
        ssx_windows_owner::read_confidential_file(&path, 32).unwrap(),
        ConfidentialRead::Bytes(_)
    ));
    let alias = target.join("hardlink.json");
    std::fs::hard_link(&path, &alias).unwrap();
    assert!(ssx_windows_owner::read_confidential_file(&path, 32).is_err());
    std::fs::remove_file(&alias).unwrap();
    let junction = root.join("junction");
    assert!(
        std::process::Command::new("cmd.exe")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&target)
            .status()
            .unwrap()
            .success()
    );
    assert!(ssx_windows_owner::read_confidential_file(&junction.join("oauth.json"), 32).is_err());
    assert!(
        ssx_windows_owner::write_confidential_file(&junction.join("new.json"), b"{}", [3; 16])
            .is_err()
    );
    fixture_public_access(&target, true).unwrap();
    assert!(ssx_windows_owner::read_confidential_file(&path, 32).is_err());
    assert!(ssx_windows_owner::write_confidential_file(&path, b"{}", [4; 16]).is_err());
}

#[test]
fn windows_installed_digest_reparse_and_writer_acl_fail_before_helper_admission() {
    for mode in ["digest", "writer", "junction"] {
        let root = root(&format!("package-{mode}"));
        let executable = stage(&root, false);
        assert!(ssx_mcp::gateway_client::resolver::install::discover(&executable).is_ok());
        let prefix = executable.parent().unwrap().parent().unwrap();
        let dns = prefix.join("libexec/slopsearx/ssx-gateway-dns.exe");
        match mode {
            "digest" => {
                let mut f = std::fs::OpenOptions::new().append(true).open(&dns).unwrap();
                f.write_all(b"tampered").unwrap();
            }
            "writer" => fixture_public_access(&dns, true).unwrap(),
            "junction" => {
                let directory = prefix.join("libexec/slopsearx");
                let moved = root.join("moved-helper");
                std::fs::rename(&directory, &moved).unwrap();
                assert!(
                    std::process::Command::new("cmd.exe")
                        .args(["/C", "mklink", "/J"])
                        .arg(&directory)
                        .arg(&moved)
                        .status()
                        .unwrap()
                        .success()
                );
            }
            _ => unreachable!(),
        }
        assert!(ssx_mcp::gateway_client::resolver::install::discover(&executable).is_err());
    }
}

#[tokio::test(flavor = "current_thread")]
async fn windows_atomic_job_assignment_failure_never_runs_child_and_valid_control_retires() {
    let root = root("job-failure");
    let executable = stage(&root, true);
    let path = executable
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("libexec/slopsearx/ssx-gateway-dns.exe");
    let image = TrustedImage::open(
        path.clone(),
        build_file_digest(&path).unwrap().sha256,
        ImageRole::Dns,
    )
    .unwrap();
    let (mut child, mut pipes) = NativeChild::dns(&image).unwrap();
    child.resume().unwrap();
    tokio::time::timeout(BUDGET, pipes.connected())
        .await
        .unwrap()
        .unwrap();
    let mut magic = [0; 16];
    tokio::time::timeout(BUDGET, pipes.stdout.read_exact(&mut magic))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&magic, b"SSX-RESOLVER-v2\0");
    assert!(child.retire(BUDGET).await.unwrap().complete());
    assert_eq!(
        NativeChild::fixture_reject_wrong_job(&image).unwrap(),
        6,
        "kernel rejects a live non-Job handle; no ResumeThread path"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn windows_abrupt_original_parent_death_closes_last_job_handle_and_retires_entered_child() {
    let root = root("parent-death");
    let executable = stage(&root, true);
    let mut parent = tokio::process::Command::new(env!("CARGO_BIN_EXE_ssx-gateway-owner-probe"))
        .arg(&executable)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut output = BufReader::new(parent.stdout.take().unwrap()).lines();
    let line = tokio::time::timeout(BUDGET, output.next_line())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let values: Vec<u64> = line
        .split_whitespace()
        .map(|v| v.parse().unwrap())
        .collect();
    assert_eq!(values.len(), 2);
    let original =
        FixtureProcessWitness::capture(u32::try_from(values[0]).unwrap(), values[1]).unwrap();
    assert!(original.running().unwrap());
    // Kill only the original owned parent Child. The witness holds no Job handle.
    parent.start_kill().unwrap();
    assert!(
        !tokio::time::timeout(BUDGET, parent.wait())
            .await
            .unwrap()
            .unwrap()
            .success()
    );
    original.exited(BUDGET).await.unwrap();
}

#[test]
fn windows_held_confidential_read_atomic_replace_and_parent_rename_have_coherent_kernel_outcomes() {
    use ssx_windows_owner::{
        FixtureConfidentialRead, read_confidential_file, write_confidential_file,
    };
    let root = root("token-overlap");
    let directory = root.join("held-parent");
    ssx_windows_owner::create_private_directory(&directory).unwrap();
    let path = directory.join("oauth.json");
    let old = br#"{"tokens":{"access_token":"old-complete-fixture"}}"#;
    let new = br#"{"tokens":{"access_token":"new-complete-fixture-with-different-length"}}"#;
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 0);
    // Initial atomic publication itself retains a production parent guard.
    write_confidential_file(&path, old, [5; 16]).unwrap();
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
    // Paired positive prevents an always-denied fixture read from qualifying.
    match FixtureConfidentialRead::open(&path)
        .unwrap()
        .read(4096)
        .unwrap()
    {
        ConfidentialRead::Bytes(bytes) => assert_eq!(bytes, old),
        _ => panic!("positive original read did not return the complete document"),
    }
    let held = FixtureConfidentialRead::open(&path).unwrap();
    // The kernel rename opens its destination parent with FILE_ADD_FILE. This
    // exact access must coexist with the held production read's parent guard.
    {
        use std::os::windows::fs::OpenOptionsExt;
        let add_file = std::fs::OpenOptions::new()
            .access_mode(0x0000_0002) // FILE_ADD_FILE
            .share_mode(0x0000_0007) // READ | WRITE | DELETE
            .custom_flags(0x0200_0000) // FILE_FLAG_BACKUP_SEMANTICS
            .open(&directory)
            .unwrap();
        drop(add_file);
    }
    // Actual kernel file + directory handles remain open during publication.
    write_confidential_file(&path, new, [6; 16]).unwrap();
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
    ssx_windows_owner::check_confidential_file(&path).unwrap();
    match read_confidential_file(&path, 4096).unwrap() {
        ConfidentialRead::Bytes(bytes) => assert_eq!(bytes, new),
        _ => panic!("fresh pathname did not publish the complete new document"),
    }
    let moved = root.join("moved-parent");
    let denial = std::fs::rename(&directory, &moved).unwrap_err();
    assert!(
        matches!(denial.raw_os_error(), Some(5 | 32)),
        "held parent must deny replacement through Windows access/sharing checks"
    );
    assert!(directory.is_dir() && !moved.exists());
    // Windows may report the replaced original as delete-pending/link-retired.
    // Its exact production identity check must deny that state, never publish
    // partial bytes or silently switch the held reader to the new pathname.
    match held.read(4096) {
        Ok(ConfidentialRead::Bytes(bytes)) => assert_eq!(bytes, old),
        Err(error) => assert_eq!(error.kind(), std::io::ErrorKind::InvalidData),
        _ => panic!("held read neither returned old-complete nor classified identity denial"),
    }
    // Releasing the original guard really removes the parent sharing fence.
    std::fs::rename(&directory, &moved).unwrap();
    ssx_windows_owner::check_confidential_file(&moved.join("oauth.json")).unwrap();
}
