//! One authenticated O02 retirement; historical inventory bytes remain unchanged.
use crate::cli_source_custody::{
    CURRENT_LOCK_BYTES, CURRENT_LOCK_SHA, HEALTH_LOCK_BYTES, HEALTH_LOCK_SHA,
    portal_health_lock_frame,
};
use sha2::{Digest, Sha256};
#[cfg(unix)]
use std::io::Read;
use std::{collections::BTreeSet, fs, path::Path};
#[cfg(test)]
use std::{ffi::OsString, path::PathBuf};
type Result<T> = std::result::Result<T, String>;
pub(crate) const PATH: &str = "docker/healthcheck.py";
const BLOB: &str = "39bcbc736bed7e441531ab00f3e6d540e4b3af65";
fn need(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(format!("O02 health retirement: {message}"))
    }
}
fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
#[cfg(test)]
fn validate_temp_root(root: &Path) -> Result<PathBuf> {
    need(
        root.is_absolute(),
        "configured test temporary root must be absolute",
    )?;
    let metadata = fs::symlink_metadata(root).map_err(|e| e.to_string())?;
    need(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "configured test temporary root must be an existing non-symlink directory",
    )?;
    let canonical = root.canonicalize().map_err(|e| e.to_string())?;
    need(
        canonical.is_absolute(),
        "configured test temporary root must resolve absolutely",
    )?;
    Ok(canonical)
}
#[cfg(test)]
fn configured_temp_root(
    tmp_dir: Option<OsString>,
    fixture_root: Option<OsString>,
) -> Result<PathBuf> {
    let tmp_dir = tmp_dir.ok_or("TMPDIR must be configured for O02 source controls")?;
    let fixture_root = fixture_root.ok_or("SLOPSEARX_TEST_TEMP_ROOT must be configured")?;
    let tmp_dir = validate_temp_root(Path::new(&tmp_dir))?;
    let fixture_root = validate_temp_root(Path::new(&fixture_root))?;
    need(
        tmp_dir == fixture_root,
        "TMPDIR and SLOPSEARX_TEST_TEMP_ROOT must resolve to the same directory",
    )?;
    Ok(tmp_dir)
}
#[cfg(unix)]
fn material(root: &Path, path: &str, size: usize, sha: &str) -> Result<Vec<u8>> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
    let full = root.join(path);
    let before = fs::symlink_metadata(&full).map_err(|e| e.to_string())?;
    need(
        before.is_file()
            && !before.file_type().is_symlink()
            && before.len() == size as u64
            && before.permissions().mode() & 0o7777 == 0o644,
        &format!("exact regular0644 source leaf: {path}"),
    )?;
    need(
        full.canonicalize()
            .map_err(|e| e.to_string())?
            .starts_with(root.canonicalize().map_err(|e| e.to_string())?),
        "material stays within root",
    )?;
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(&full)
        .map_err(|e| e.to_string())?;
    let opened = file.metadata().map_err(|e| e.to_string())?;
    need(
        opened.is_file()
            && opened.len() == size as u64
            && opened.permissions().mode() & 0o7777 == 0o644
            && before.dev() == opened.dev()
            && before.ino() == opened.ino(),
        "descriptor-bound regular0644 source identity",
    )?;
    let mut raw = Vec::new();
    (&file)
        .take(size as u64 + 1)
        .read_to_end(&mut raw)
        .map_err(|e| e.to_string())?;
    let after = file.metadata().map_err(|e| e.to_string())?;
    let named = fs::symlink_metadata(&full).map_err(|e| e.to_string())?;
    need(
        after.is_file()
            && after.len() == size as u64
            && after.permissions().mode() & 0o7777 == 0o644
            && after.dev() == opened.dev()
            && after.ino() == opened.ino()
            && named.is_file()
            && !named.file_type().is_symlink()
            && named.len() == after.len()
            && named.permissions().mode() & 0o7777 == 0o644
            && named.dev() == after.dev()
            && named.ino() == after.ino(),
        "source identity unchanged after bounded read",
    )?;
    need(
        raw.len() == size && digest(&raw) == sha,
        "exact material bytes",
    )?;
    Ok(raw)
}
#[cfg(not(unix))]
fn material(_: &Path, _: &str, _: usize, _: &str) -> Result<Vec<u8>> {
    Err("O02 health retirement source proof requires a Unix qualification host".into())
}
fn absence(root: &Path, tracked: &BTreeSet<String>) -> Result<()> {
    need(!tracked.contains(PATH), "retired path remains untracked")?;
    let parent = root.join("docker");
    match fs::symlink_metadata(&parent) {
        Ok(meta) => {
            need(
                meta.is_dir() && !meta.file_type().is_symlink(),
                "retired parent requires actual directory",
            )?;
            need(
                parent
                    .canonicalize()
                    .map_err(|e| e.to_string())?
                    .starts_with(root.canonicalize().map_err(|e| e.to_string())?),
                "retired parent stays within root",
            )?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            need(
                !tracked.iter().any(|path| path.starts_with("docker/")),
                "absent retired parent has no indexed descendants",
            )?;
        }
        Err(error) => return Err(error.to_string()),
    }
    match fs::symlink_metadata(root.join(PATH)) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        _ => Err("O02 health retirement: retired path must be physically absent".into()),
    }
}
fn historical(fixture: &[u8], disposition: &[u8]) -> Result<()> {
    need(
        fixture.len() == 64639
            && digest(fixture)
                == "eeb28862df43b91b1eb6d5e83255e9b1a1c1935b144b5cafcb0ed59fec3239a3",
        "unchanged F01 fixture",
    )?;
    need(
        disposition.len() == 422610
            && digest(disposition)
                == "8b3d95d5d4f7cb9c4ba87f8a168216135480e0ffb4e13235372f62d5b366b29a",
        "unchanged Python dispositions",
    )?;
    let fixture: serde_json::Value = serde_json::from_slice(fixture).map_err(|e| e.to_string())?;
    let rows = fixture["sources"].as_array().ok_or("F01 sources")?;
    let retired: Vec<_> = rows.iter().filter(|row| row["path"] == PATH).collect();
    need(
        retired.len() == 1 && retired[0]["git_blob"] == BLOB,
        "sole historical source identity",
    )?;
    let disposition: serde_json::Value =
        serde_json::from_slice(disposition).map_err(|e| e.to_string())?;
    let rows = disposition["files"]
        .as_array()
        .ok_or("Python dispositions")?;
    let retired: Vec<_> = rows.iter().filter(|row| row["path"] == PATH).collect();
    need(
        retired.len() == 1
            && retired[0]["classification"] == "backend/runtime"
            && retired[0]["migration_owner"] == "O02",
        "sole historical disposition owner",
    )
}
pub(crate) fn authenticate(root: &Path, fixture: &[u8], tracked: &BTreeSet<String>) -> Result<()> {
    let disposition = material(
        root,
        "docs/rust-rewrite/inventory/python-disposition.json",
        422610,
        "8b3d95d5d4f7cb9c4ba87f8a168216135480e0ffb4e13235372f62d5b366b29a",
    )?;
    historical(fixture, &disposition)?;
    absence(root, tracked)?;
    for (path, size, sha) in [
        (
            "crates/ssx-healthcheck/Cargo.toml",
            191,
            "8362e555da02dd64d7d48b4837fa16da5ee3732f535d2ba4be1591e84f8fde77",
        ),
        (
            "crates/ssx-healthcheck/src/lib.rs",
            5985,
            "8704602c99d4ccb0cf8169dc6e07151b8c5af1015f9cb4ef5c1248016da5859f",
        ),
        (
            "crates/ssx-healthcheck/src/main.rs",
            253,
            "25afa196c3537ec8c270da8d04982ec9ecd58d33c6ddcd96ddd6ef6a8d9d81c2",
        ),
        (
            "Dockerfile",
            2427,
            "05bb1d6087f15b85904283e6cac78582465169f4fa43f2f5f681390a09dc3607",
        ),
    ] {
        material(root, path, size, sha)?;
    }
    let current = material(root, "Cargo.lock", CURRENT_LOCK_BYTES, CURRENT_LOCK_SHA)?;
    let health = portal_health_lock_frame(&current)?;
    need(
        health.len() == HEALTH_LOCK_BYTES && digest(&health) == HEALTH_LOCK_SHA,
        "exact accepted health lock after authenticated portal frame",
    )?;
    Ok(())
}
#[cfg(test)]
pub(crate) fn controls(root: &Path) -> Result<()> {
    let temp_root = configured_temp_root(
        std::env::var_os("TMPDIR"),
        std::env::var_os("SLOPSEARX_TEST_TEMP_ROOT"),
    )?;
    need(
        configured_temp_root(None, std::env::var_os("SLOPSEARX_TEST_TEMP_ROOT")).is_err(),
        "missing TMPDIR rejected",
    )?;
    need(
        configured_temp_root(std::env::var_os("TMPDIR"), None).is_err(),
        "missing test temporary root rejected",
    )?;
    need(
        validate_temp_root(Path::new("relative-temp-root")).is_err(),
        "relative configured temp root rejected",
    )?;
    let fixture =
        fs::read(root.join("crates/xtask/fixtures/F01.json")).map_err(|e| e.to_string())?;
    let disposition = fs::read(root.join("docs/rust-rewrite/inventory/python-disposition.json"))
        .map_err(|e| e.to_string())?;
    historical(&fixture, &disposition)?;
    // The original fixture is retained, not replaced with a reduced inventory.
    for duplicate in [false, true] {
        let mut doc: serde_json::Value =
            serde_json::from_slice(&fixture).map_err(|e| e.to_string())?;
        let rows = doc["sources"].as_array_mut().ok_or("F01 sources")?;
        let row = rows
            .iter()
            .find(|row| row["path"] == PATH)
            .ok_or("retired row")?
            .clone();
        if duplicate {
            rows.push(row);
        } else {
            rows.retain(|row| row["path"] != PATH);
        }
        need(
            historical(
                &serde_json::to_vec(&doc).map_err(|e| e.to_string())?,
                &disposition,
            )
            .is_err(),
            "missing/duplicate retirement rejected",
        )?;
    }
    let mut changed = fixture.clone();
    changed[0] ^= 1;
    need(
        historical(&changed, &disposition).is_err(),
        "changed frozen source rejected",
    )?;
    for row in SUCCESSORS
        .iter()
        .filter(|row| row.0.starts_with("scripts/"))
    {
        let raw = material(root, row.0, row.2, row.3)?;
        let old = prior(row, &raw)?;
        need(
            prior(row, &old).is_err(),
            "historical bytes are not current",
        )?;
        let mut changed = raw.clone();
        changed[0] ^= 1;
        need(
            prior(row, &changed).is_err(),
            "changed Python successor rejected",
        )?;
        let mut duplicate = raw.clone();
        duplicate.extend_from_slice(&raw);
        need(
            prior(row, &duplicate).is_err(),
            "duplicate Python successor rejected",
        )?;
        need(
            prior(row, &[]).is_err(),
            "missing Python successor rejected",
        )?;
        need(
            successor(root, row.0, "wrong").is_err(),
            "wrong historical blob rejected",
        )?;
    }
    let owned = temp_root.join(format!(
        "ssx-health-retirement-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos()
    ));
    fs::create_dir(&owned).map_err(|e| e.to_string())?;
    let result = (|| -> Result<()> {
        let alternate_root = owned.join("alternate-temp-root");
        fs::create_dir(&alternate_root).map_err(|e| e.to_string())?;
        need(
            configured_temp_root(
                std::env::var_os("TMPDIR"),
                Some(alternate_root.clone().into_os_string()),
            )
            .is_err(),
            "mismatched configured temp roots rejected",
        )?;
        fs::remove_dir(&alternate_root).map_err(|e| e.to_string())?;
        let file_root = owned.join("not-a-directory");
        fs::write(&file_root, b"file").map_err(|e| e.to_string())?;
        need(
            validate_temp_root(&file_root).is_err(),
            "non-directory configured temp root rejected",
        )?;
        fs::remove_file(&file_root).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            let linked_root = owned.join("linked-temp-root");
            std::os::unix::fs::symlink(&owned, &linked_root).map_err(|e| e.to_string())?;
            need(
                validate_temp_root(&linked_root).is_err(),
                "symlinked configured temp root rejected",
            )?;
            fs::remove_file(&linked_root).map_err(|e| e.to_string())?;
        }
        // Fresh Git checkouts omit the directory after its last tracked file retires.
        absence(&owned, &BTreeSet::new())?;
        need(
            absence(
                &owned,
                &BTreeSet::from(["docker/unexpected.py".to_string()]),
            )
            .is_err(),
            "missing parent with indexed descendant rejected",
        )?;
        fs::create_dir(owned.join("docker")).map_err(|e| e.to_string())?;
        // Exercise the real fixed replacement authentication on an owned closure,
        // then mutate the actual native source material, not its expected tuple.
        for path in [
            "docs/rust-rewrite/inventory/python-disposition.json",
            "crates/ssx-healthcheck/Cargo.toml",
            "crates/ssx-healthcheck/src/lib.rs",
            "crates/ssx-healthcheck/src/main.rs",
            "Dockerfile",
            "Cargo.lock",
        ] {
            let destination = owned.join(path);
            fs::create_dir_all(destination.parent().ok_or("replacement parent")?)
                .map_err(|e| e.to_string())?;
            fs::copy(root.join(path), destination).map_err(|e| e.to_string())?;
        }
        authenticate(&owned, &fixture, &BTreeSet::new())?;
        let lock = owned.join("Cargo.lock");
        let original_lock = fs::read(&lock).map_err(|e| e.to_string())?;
        let mut changed_lock = original_lock.clone();
        changed_lock[0] ^= 1;
        fs::write(&lock, changed_lock).map_err(|e| e.to_string())?;
        need(
            authenticate(&owned, &fixture, &BTreeSet::new()).is_err(),
            "changed shared Cargo.lock identity rejected",
        )?;
        fs::write(&lock, &original_lock).map_err(|e| e.to_string())?;
        authenticate(&owned, &fixture, &BTreeSet::new())?;
        let native = owned.join("crates/ssx-healthcheck/src/lib.rs");
        let original = fs::read(&native).map_err(|e| e.to_string())?;
        let mut changed = original.clone();
        changed[0] ^= 1;
        fs::write(&native, &changed).map_err(|e| e.to_string())?;
        need(
            authenticate(&owned, &fixture, &BTreeSet::new()).is_err(),
            "changed actual native replacement rejected",
        )?;
        fs::remove_file(&native).map_err(|e| e.to_string())?;
        need(
            authenticate(&owned, &fixture, &BTreeSet::new()).is_err(),
            "missing actual native replacement rejected",
        )?;
        fs::copy(root.join("crates/ssx-healthcheck/src/lib.rs"), &native)
            .map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            fs::remove_file(&native).map_err(|e| e.to_string())?;
            std::os::unix::fs::symlink(root.join("crates/ssx-healthcheck/src/lib.rs"), &native)
                .map_err(|e| e.to_string())?;
            need(
                authenticate(&owned, &fixture, &BTreeSet::new()).is_err(),
                "symlinked native replacement rejected",
            )?;
            fs::remove_file(&native).map_err(|e| e.to_string())?;
            fs::copy(root.join("crates/ssx-healthcheck/src/lib.rs"), &native)
                .map_err(|e| e.to_string())?;
        }
        authenticate(&owned, &fixture, &BTreeSet::new())?;
        fs::write(owned.join(PATH), b"recreated retired source").map_err(|e| e.to_string())?;
        need(
            absence(&owned, &BTreeSet::new()).is_err(),
            "recreated retired source rejected",
        )?;
        fs::remove_file(owned.join(PATH)).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("missing-target", owned.join(PATH))
                .map_err(|e| e.to_string())?;
            need(
                absence(&owned, &BTreeSet::new()).is_err(),
                "dangling retired symlink rejected",
            )?;
            fs::remove_file(owned.join(PATH)).map_err(|e| e.to_string())?;
            fs::rename(owned.join("docker"), owned.join("docker-real"))
                .map_err(|e| e.to_string())?;
            std::os::unix::fs::symlink("docker-real", owned.join("docker"))
                .map_err(|e| e.to_string())?;
            need(
                absence(&owned, &BTreeSet::new()).is_err(),
                "symlinked retired parent rejected",
            )?;
        }
        Ok(())
    })();
    let cleanup = fs::remove_dir_all(&owned).map_err(|e| e.to_string());
    match (result, cleanup) {
        (Ok(()), Ok(())) => {}
        (Err(error), Ok(())) => return Err(error),
        (Ok(()), Err(cleanup)) => {
            return Err(format!("O02 temporary control cleanup failed: {cleanup}"));
        }
        (Err(error), Err(cleanup)) => {
            return Err(format!(
                "{error}; O02 temporary control cleanup also failed: {cleanup}"
            ));
        }
    }
    let tracked = BTreeSet::from([PATH.to_string()]);
    need(
        absence(root, &tracked).is_err(),
        "tracked retired source rejected",
    )?;
    Ok(())
}

// Exact two interim Python source successors; neither is executed by this proof.
type Successor = (
    &'static str,
    &'static str,
    usize,
    &'static str,
    usize,
    &'static str,
    &'static [(&'static str, &'static str)],
);
const SUCCESSORS: [Successor; 2] = [
    (
        "scripts/verify_docker_base_digest.py",
        "3f43f9bc1f90b4b2cd85381d2a54e54387cfab18",
        5548,
        "835207d44b4fe5871a356f1953d1850ee73a8c639ca9ebf9bd7c26de04cb2fd5",
        4509,
        "2de0aba47a2a8fa667c7b33169dbcb5c2a5ad0faaf7424511127e832e70843fb",
        &[
            (
                "#!/usr/bin/env python3\n\"\"\"Verify that Dockerfile's pinned Python digest matches Docker Hub.\"\"\"\n\nimport argparse\nimport json\n",
                "#!/usr/bin/env python3\n\"\"\"Verify every explicitly supported Docker build/runtime base against Docker Hub.\"\"\"\n\nimport argparse\nimport json\n",
            ),
            (
                "    raise RegistryLookupError(f\"{label} failed with {detail} after {attempts} attempt(s)\") from last_error\n\n\ndef _pinned_python_base(dockerfile: str) -> tuple[str, str]:\n    from_lines = list(ALL_FROM_RE.finditer(dockerfile))\n    if len(from_lines) != 1:\n        raise SystemExit(\"Dockerfile must contain exactly one FROM line for base verification\")\n\n    match = FROM_RE.fullmatch(from_lines[0].group(0))\n    if match is None:\n        raise SystemExit(\"Dockerfile must pin python base image by tag and digest\")\n    return match.group(\"tag\"), match.group(\"digest\")\n\n\ndef registry_digest(repository: str, tag: str) -> str:\n",
                "    raise RegistryLookupError(f\"{label} failed with {detail} after {attempts} attempt(s)\") from last_error\n\n\nRUST_FROM_RE = re.compile(\n    r\"^\\s*FROM\\s+rust:(?P<tag>1\\.99\\.0-bookworm)@(?P<digest>sha256:[0-9a-f]{64})\\s+AS\\s+healthcheck-builder\\s*$\",\n    re.IGNORECASE,\n)\n\n\ndef pinned_bases(dockerfile: str) -> list[tuple[str, str, str]]:\n    lines = [match.group(0) for match in ALL_FROM_RE.finditer(dockerfile)]\n    if len(lines) == 1:\n        runtime = FROM_RE.fullmatch(lines[0])\n        if runtime is None:\n            raise SystemExit(\"Dockerfile must pin python base image by tag and digest\")\n        return [(\"library/python\", runtime.group(\"tag\"), runtime.group(\"digest\"))]\n    if len(lines) != 2:\n        raise SystemExit(\"Dockerfile must contain exactly one runtime FROM or two recognized FROM stages\")\n    builder = RUST_FROM_RE.fullmatch(lines[0])\n    runtime = FROM_RE.fullmatch(lines[1])\n    if builder is None or runtime is None or len(lines[1].split()) != 2:\n        raise SystemExit(\"Dockerfile must pin the recognized native health builder followed by Python runtime\")\n    return [\n        (\"library/rust\", builder.group(\"tag\"), builder.group(\"digest\")),\n        (\"library/python\", runtime.group(\"tag\"), runtime.group(\"digest\")),\n    ]\n\n\ndef _pinned_python_base(dockerfile: str) -> tuple[str, str]:\n    _, tag, digest = pinned_bases(dockerfile)[-1]\n    return tag, digest\n\n\ndef registry_digest(repository: str, tag: str) -> str:\n",
            ),
            (
                "            ),\n        },\n    )\n    with _urlopen_with_retries(request, label=f\"Docker Hub manifest request for python:{tag}\") as response:\n        digest = response.headers.get(\"Docker-Content-Digest\", \"\")\n    if not DIGEST_RE.fullmatch(digest):\n        raise RegistryLookupError(\"Docker Hub did not return a valid manifest digest\")\n",
                "            ),\n        },\n    )\n    with _urlopen_with_retries(request, label=f\"Docker Hub manifest request for {repository}:{tag}\") as response:\n        digest = response.headers.get(\"Docker-Content-Digest\", \"\")\n    if not DIGEST_RE.fullmatch(digest):\n        raise RegistryLookupError(\"Docker Hub did not return a valid manifest digest\")\n",
            ),
            (
                "    args = parser.parse_args()\n\n    try:\n        tag, expected = _pinned_python_base(Path(args.dockerfile).read_text())\n        actual = registry_digest(\"library/python\", tag)\n    except OSError as exc:\n        print(f\"Unable to read Dockerfile: {exc}\", file=sys.stderr)\n        return 2\n",
                "    args = parser.parse_args()\n\n    try:\n        bases = pinned_bases(Path(args.dockerfile).read_text())\n        observations = [\n            (repository, tag, expected, registry_digest(repository, tag)) for repository, tag, expected in bases\n        ]\n    except OSError as exc:\n        print(f\"Unable to read Dockerfile: {exc}\", file=sys.stderr)\n        return 2\n",
            ),
            (
                "        print(f\"Docker Hub registry lookup failed: {exc}\", file=sys.stderr)\n        return 2\n\n    print(f\"python:{tag} expected={expected} registry={actual}\")\n    if expected != actual:\n        print(\"Pinned Docker base digest is stale; update tag and digest together.\", file=sys.stderr)\n        return 1\n    return 0\n",
                "        print(f\"Docker Hub registry lookup failed: {exc}\", file=sys.stderr)\n        return 2\n\n    for repository, tag, expected, actual in observations:\n        print(f\"{repository}:{tag} expected={expected} registry={actual}\")\n    if any(expected != actual for _, _, expected, actual in observations):\n        print(\"Pinned Docker base digest is stale; update tag and digest together.\", file=sys.stderr)\n        return 1\n    return 0\n",
            ),
        ],
    ),
    (
        "tests/test_deployment_artifacts.py",
        "649650e35656bfff78d7930052085270811343fd",
        12945,
        "78d9dfccbb97dca38a4149e5f5154ebae18a11589616492234d9817e9d2ea1e2",
        12872,
        "e7634a07ab53f6409386a9515f46ab65ebccfb0747235d6ad43a07dcf54f431c",
        &[(
            "    workflow = (ROOT / \".github/workflows/docker.yml\").read_text()\n    assert \"python3 scripts/verify_docker_base_digest.py\" in workflow\n",
            "    workflow = (ROOT / \".github/workflows/docker.yml\").read_text()\n    assert \"cargo xtask ci-helper image-pin-verify-base\" in workflow\n    assert \"python3 scripts/verify_docker_base_digest.py\" not in workflow\n",
        )],
    ),
];
fn prior(row: &Successor, raw: &[u8]) -> Result<Vec<u8>> {
    need(
        raw.len() == row.2 && digest(raw) == row.3,
        "exact current Python successor",
    )?;
    let current = std::str::from_utf8(raw).map_err(|e| e.to_string())?;
    let mut old = current.to_string();
    for &(before, after) in row.6 {
        need(
            old.matches(after).count() == 1,
            "unique inverse Python successor span",
        )?;
        old = old.replacen(after, before, 1);
    }
    need(
        old.len() == row.4 && digest(old.as_bytes()) == row.5,
        "exact historical Python operand",
    )?;
    let mut forward = old.clone();
    for &(before, after) in row.6 {
        need(
            forward.matches(before).count() == 1,
            "unique forward Python successor span",
        )?;
        forward = forward.replacen(before, after, 1);
    }
    need(
        forward.as_bytes() == raw,
        "complete Python successor reconstruction",
    )?;
    Ok(old.into_bytes())
}
pub(crate) fn successor(root: &Path, path: &str, blob: &str) -> Result<bool> {
    let Some(row) = SUCCESSORS.iter().find(|row| row.0 == path) else {
        return Ok(false);
    };
    need(row.1 == blob, "historical Python fixture blob")?;
    let raw = if path == "tests/test_deployment_artifacts.py" {
        bounded_current_python_source(root, row)?
    } else {
        material(root, row.0, row.2, row.3)?
    };
    prior(row, &raw)?;
    Ok(true)
}
fn bounded_current_python_source(root: &Path, row: &Successor) -> Result<Vec<u8>> {
    // The successor tuple already fixes the only admissible bytes. Reuse the
    // descriptor-bound reader so replacement/growth cannot bypass the read cap.
    material(root, row.0, row.2, row.3)
}
/// Current deployment-test source projected through the same strict F01 inverse.
pub(crate) fn historical_python_source(
    root: &Path,
    row: &serde_json::Value,
) -> Result<Option<Vec<u8>>> {
    let path = row["path"].as_str().ok_or("O02 historical source path")?;
    if path != "tests/test_deployment_artifacts.py" {
        return Ok(None);
    }
    let fields = row.as_object().ok_or("O02 historical source row")?;
    need(
        fields.len() == 2 && fields.contains_key("path") && fields.contains_key("sha256"),
        "closed historical Python source row",
    )?;
    let source = SUCCESSORS
        .iter()
        .find(|source| source.0 == path)
        .ok_or("O02 source tuple")?;
    if row["sha256"].as_str()
        == Some("ce83065f173b7d05c7790a8aaae55efec4fd8eb18ed69c5e534afcc7934f8a2b")
    {
        need(fields.len() == 2, "closed original W03 Python source pin")?;
        let output = std::process::Command::new("git")
            .args([
                "show",
                "87af8096d46359241421ac747ed6aa1ad07f38ec:tests/test_deployment_artifacts.py",
            ])
            .current_dir(root)
            .output()
            .map_err(|error| error.to_string())?;
        need(
            output.status.success(),
            "W03 historical source object is available",
        )?;
        need(
            output.stdout.len() == 11133
                && digest(&output.stdout)
                    == "ce83065f173b7d05c7790a8aaae55efec4fd8eb18ed69c5e534afcc7934f8a2b",
            "W03 historical source bytes match immutable pin",
        )?;
        let output_sha = digest(&output.stdout);
        need(
            row["sha256"].as_str() == Some(output_sha.as_str()),
            "W03 historical source row binds returned bytes",
        )?;
        return Ok(Some(output.stdout));
    }
    if row["sha256"].as_str() != Some(source.5) {
        return Err(
            "historical Python source hash is neither the O02 successor nor W03 predecessor".into(),
        );
    }
    let raw = bounded_current_python_source(root, source)?;
    prior(source, &raw).map(Some)
}

#[cfg(all(test, unix))]
mod bounded_source_tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

    #[test]
    fn current_python_source_requires_authenticated_regular_material() {
        let base = configured_temp_root(
            std::env::var_os("TMPDIR"),
            std::env::var_os("SLOPSEARX_TEST_TEMP_ROOT"),
        )
        .unwrap();
        let root = base.join(format!("ssx-current-source-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let row = SUCCESSORS
            .iter()
            .find(|row| row.0 == "tests/test_deployment_artifacts.py")
            .unwrap();
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(row.0);
        let path = root.join(row.0);
        fs::create_dir(path.parent().unwrap()).unwrap();
        fs::copy(&source, &path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        let expected = fs::read(&source).unwrap();
        assert_eq!(bounded_current_python_source(&root, row).unwrap(), expected);

        let mut corrupt = expected.clone();
        corrupt[0] ^= 1;
        fs::write(&path, &corrupt).unwrap();
        assert!(bounded_current_python_source(&root, row).is_err());
        fs::write(&path, &expected).unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(131073)
            .unwrap();
        assert!(bounded_current_python_source(&root, row).is_err());
        fs::remove_file(&path).unwrap();
        symlink(&source, &path).unwrap();
        assert!(bounded_current_python_source(&root, row).is_err());
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(bounded_current_python_source(&root, row).is_err());
        fs::remove_dir_all(&root).unwrap();
    }
}
