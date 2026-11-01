//! Two authenticated O02 retirements; historical inventory bytes remain unchanged.
use sha2::{Digest, Sha256};
#[cfg(unix)]
use std::io::Read;
use std::{collections::BTreeSet, fs, path::Path};
type Result<T> = std::result::Result<T, String>;
pub(crate) const PATH: &str = "docker/healthcheck.py";
pub(crate) const VERIFIER: &str = "scripts/verify_docker_base_digest.py";
const VERIFIER_BLOB: &str = "3f43f9bc1f90b4b2cd85381d2a54e54387cfab18";
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
        "exact regular0644 source leaf",
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
fn absence_of(root: &Path, tracked: &BTreeSet<String>, path: &str) -> Result<()> {
    need(path == PATH || path == VERIFIER, "closed retired path")?;
    need(!tracked.contains(path), "retired path remains untracked")?;
    let relative_parent = Path::new(path).parent().ok_or("retired parent")?;
    let prefix = format!("{}/", relative_parent.display());
    let parent = root.join(relative_parent);
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
                !tracked.iter().any(|path| path.starts_with(&prefix)),
                "absent retired parent has no indexed descendants",
            )?;
        }
        Err(error) => return Err(error.to_string()),
    }
    match fs::symlink_metadata(root.join(path)) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        _ => Err("O02 health retirement: retired path must be physically absent".into()),
    }
}
fn absence(root: &Path, tracked: &BTreeSet<String>) -> Result<()> {
    absence_of(root, tracked, PATH)
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
    let verifier: Vec<_> = rows.iter().filter(|row| row["path"] == VERIFIER).collect();
    need(
        verifier.len() == 1 && verifier[0]["git_blob"] == VERIFIER_BLOB,
        "sole historical verifier identity",
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
    )?;
    let verifier: Vec<_> = rows.iter().filter(|row| row["path"] == VERIFIER).collect();
    need(
        verifier.len() == 1
            && verifier[0]["classification"] == "backend-coupled tooling/test"
            && verifier[0]["migration_owner"] == "O02",
        "sole historical verifier disposition",
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
    absence_of(root, tracked, VERIFIER)?;
    image_operations(root)?;
    for (path, size, sha) in [
        (
            "crates/ssx-healthcheck/Cargo.toml",
            191,
            "8362e555da02dd64d7d48b4837fa16da5ee3732f535d2ba4be1591e84f8fde77",
        ),
        (
            "crates/ssx-healthcheck/src/lib.rs",
            5122,
            "58d78e3128f0c01642df53bb20ad2d821a191664eb602381e4da0784eb45c14a",
        ),
        (
            "crates/ssx-healthcheck/src/main.rs",
            407,
            "87a5f302ea52e8d068cf150675c258ba3a4540016ee2acf360250ba8c8054e95",
        ),
        (
            "Dockerfile",
            2427,
            "05bb1d6087f15b85904283e6cac78582465169f4fa43f2f5f681390a09dc3607",
        ),
    ] {
        material(root, path, size, sha)?;
    }
    let lock = material(
        root,
        "Cargo.lock",
        94371,
        "8d8d87dc69683e47e9b28b68ee332f1b585001128905e2a26481e61c3b37d7d5",
    )?;
    let health = crate::cli_source_custody::native_image_ops_lock_prior(&lock)?;
    need(
        health.len() == 94250
            && digest(&health)
                == "53fb4619c6e4fd12eae7928073c5062929498d2db29b270e456514a855a5bdd9",
        "exact accepted health lock operand",
    )?;
    Ok(())
}
#[cfg(test)]
pub(crate) fn controls(root: &Path) -> Result<()> {
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
    for row in &SUCCESSORS {
        if row.0 == VERIFIER {
            continue;
        }
        let raw = python_successor(root, row)?;
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
    let owned = std::env::temp_dir().join(format!(
        "ssx-health-retirement-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos()
    ));
    fs::create_dir(&owned).map_err(|e| e.to_string())?;
    let result = (|| -> Result<()> {
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
            "crates/ssx-ops/Cargo.toml",
            "crates/ssx-ops/src/lib.rs",
            "crates/ssx-ops/src/main.rs",
            "crates/ssx-ops/src/tests.rs",
            "crates/ssx-ops/tests/commands.rs",
            ".github/workflows/docker.yml",
            "docs/rust-rewrite/ci/gates.json",
            "tests/test_deployment_artifacts.py",
        ] {
            let destination = owned.join(path);
            fs::create_dir_all(destination.parent().ok_or("replacement parent")?)
                .map_err(|e| e.to_string())?;
            fs::copy(root.join(path), destination).map_err(|e| e.to_string())?;
        }
        authenticate(&owned, &fixture, &BTreeSet::new())?;
        need(
            authenticate(&owned, &fixture, &BTreeSet::from([VERIFIER.to_string()])).is_err(),
            "tracked verifier rejected",
        )?;
        fs::create_dir(owned.join("scripts")).map_err(|e| e.to_string())?;
        fs::write(owned.join(VERIFIER), b"recreated verifier").map_err(|e| e.to_string())?;
        need(
            authenticate(&owned, &fixture, &BTreeSet::new()).is_err(),
            "recreated verifier rejected",
        )?;
        fs::remove_file(owned.join(VERIFIER)).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("missing", owned.join(VERIFIER))
                .map_err(|e| e.to_string())?;
            need(
                authenticate(&owned, &fixture, &BTreeSet::new()).is_err(),
                "verifier symlink rejected",
            )?;
            fs::remove_file(owned.join(VERIFIER)).map_err(|e| e.to_string())?;
            fs::rename(owned.join("scripts"), owned.join("scripts-real"))
                .map_err(|e| e.to_string())?;
            std::os::unix::fs::symlink("scripts-real", owned.join("scripts"))
                .map_err(|e| e.to_string())?;
            need(
                authenticate(&owned, &fixture, &BTreeSet::new()).is_err(),
                "verifier parent symlink rejected",
            )?;
            fs::remove_file(owned.join("scripts")).map_err(|e| e.to_string())?;
            fs::rename(owned.join("scripts-real"), owned.join("scripts"))
                .map_err(|e| e.to_string())?;
        }
        let ops_native = owned.join("crates/ssx-ops/src/lib.rs");
        let ops_original = fs::read(&ops_native).map_err(|e| e.to_string())?;
        let mut changed_ops = ops_original;
        changed_ops[0] ^= 1;
        fs::write(&ops_native, changed_ops).map_err(|e| e.to_string())?;
        need(
            authenticate(&owned, &fixture, &BTreeSet::new()).is_err(),
            "changed native verifier rejected",
        )?;
        fs::remove_file(&ops_native).map_err(|e| e.to_string())?;
        need(
            authenticate(&owned, &fixture, &BTreeSet::new()).is_err(),
            "missing native verifier rejected",
        )?;
        fs::copy(root.join("crates/ssx-ops/src/lib.rs"), &ops_native).map_err(|e| e.to_string())?;
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
    result?;
    cleanup?;
    let ops_test = material(root, OPS_TEST.0, OPS_TEST.2, OPS_TEST.3)?;
    let health_test = prior(&OPS_TEST, &ops_test)?;
    need(
        prior(&OPS_TEST, &health_test).is_err(),
        "health test bytes are not Ops current",
    )?;
    let mut changed_ops_test = ops_test.clone();
    changed_ops_test[0] ^= 1;
    need(
        prior(&OPS_TEST, &changed_ops_test).is_err(),
        "changed Ops Python test source rejected",
    )?;
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
        12872,
        "e7634a07ab53f6409386a9515f46ab65ebccfb0747235d6ad43a07dcf54f431c",
        11133,
        "ce83065f173b7d05c7790a8aaae55efec4fd8eb18ed69c5e534afcc7934f8a2b",
        &[
            (
                "    digest = \"a\" * 64\n    dockerfile = f\"FROM python:3.12-slim@sha256:{digest}\\nFROM scratch\\n\"\n\n    with pytest.raises(SystemExit, match=\"exactly one FROM\"):\n        verify._pinned_python_base(dockerfile)\n\n\n",
                "    digest = \"a\" * 64\n    dockerfile = f\"FROM python:3.12-slim@sha256:{digest}\\nFROM scratch\\n\"\n\n    with pytest.raises(SystemExit, match=\"recognized\"):\n        verify._pinned_python_base(dockerfile)\n\n\n",
            ),
            (
                "    digest = \"a\" * 64\n    dockerfile = f\"FROM python:3.12-slim@sha256:{digest}\\n{second_from}\\n\"\n\n    with pytest.raises(SystemExit, match=\"exactly one FROM\"):\n        verify._pinned_python_base(dockerfile)\n\n\n",
                "    digest = \"a\" * 64\n    dockerfile = f\"FROM python:3.12-slim@sha256:{digest}\\n{second_from}\\n\"\n\n    with pytest.raises(SystemExit, match=\"recognized\"):\n        verify._pinned_python_base(dockerfile)\n\n\n",
            ),
            (
                "    assert \"${GITHUB_SHA::7}-${{ matrix.arch }}\" in tag_command\n    assert \"GITHUB_RUN_ID\" not in tag_command\n    assert \"GITHUB_RUN_ATTEMPT\" not in tag_command\n",
                "    assert \"${GITHUB_SHA::7}-${{ matrix.arch }}\" in tag_command\n    assert \"GITHUB_RUN_ID\" not in tag_command\n    assert \"GITHUB_RUN_ATTEMPT\" not in tag_command\n\n\ndef test_health_builder_and_runtime_bases_are_both_verified(monkeypatch: pytest.MonkeyPatch) -> None:\n    dockerfile = (ROOT / \"Dockerfile\").read_text()\n    bases = verify.pinned_bases(dockerfile)\n    builder = dockerfile.split(\"# SlopSearX production image\", 1)[0]\n    assert \"COPY . .\" not in builder\n    assert [line for line in builder.splitlines() if line.startswith(\"COPY \")] == [\n        \"COPY Cargo.toml Cargo.lock rust-toolchain.toml ./\",\n        \"COPY crates/ crates/\",\n        \"COPY vendor/ vendor/\",\n    ]\n    assert [repository for repository, _, _ in bases] == [\"library/rust\", \"library/python\"]\n    assert bases[0][1] == \"1.99.0-bookworm\"\n    assert 'CMD [\"/usr/local/bin/ssx-healthcheck\"]' in dockerfile\n    assert \"healthcheck.py\" not in dockerfile\n    assert \"uvicorn\" in dockerfile  # This increment does not claim server retirement.\n    for malformed in [\n        dockerfile + \"\\nFROM scratch\\n\",\n        dockerfile.replace(\" AS healthcheck-builder\", \" AS arbitrary-builder\"),\n        dockerfile.replace(\"rust:1.99.0-bookworm@sha256:\", \"rust:1.99.0-bookworm@bad:\"),\n    ]:\n        with pytest.raises(SystemExit):\n            verify.pinned_bases(malformed)\n    calls: list[str] = []\n\n    def actual(repository: str, tag: str) -> str:\n        calls.append(repository)\n        return next(digest for repo, _, digest in bases if repo == repository)\n\n    monkeypatch.setattr(verify, \"registry_digest\", actual)\n    monkeypatch.setattr(\"sys.argv\", [\"verify_docker_base_digest.py\", \"--dockerfile\", str(ROOT / \"Dockerfile\")])\n    assert verify.main() == 0\n    assert calls == [\"library/rust\", \"library/python\"]\n    monkeypatch.setattr(verify, \"registry_digest\", lambda repository, tag: \"sha256:\" + \"0\" * 64)\n    assert verify.main() == 1\n",
            ),
        ],
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
    let raw = python_successor(root, row)?;
    prior(row, &raw)?;
    Ok(true)
}

// Closed native image-operations replacement, authenticated before retirement.
const OPS_INPUTS: &[(&str, usize, &str)] = &[
    (
        "crates/ssx-ops/Cargo.toml",
        393,
        "970910567a818ca292f7998ef6c47df887291740075b883912ab2e296eeea5e3",
    ),
    (
        "crates/ssx-ops/src/lib.rs",
        10023,
        "27d928164598ff5a8e014176323b2de82bf11477713bbf9479433fd9d030f349",
    ),
    (
        "crates/ssx-ops/src/main.rs",
        1912,
        "8a71b5282452fc292aa79a803226fac8bf958b907d6b721ff2e37b1b9b65ed8d",
    ),
    (
        "crates/ssx-ops/src/tests.rs",
        8657,
        "afdb297fcaf540379acd924af3bb70882c31e9986bf14876b7859eae0658be1d",
    ),
    (
        "crates/ssx-ops/tests/commands.rs",
        2943,
        "e1339c7db939145ca8fa3f989d897d510440044e82e69803ba647541f5a479cc",
    ),
    (
        ".github/workflows/docker.yml",
        10115,
        "975cfff7e0036464ff291d6f2412691fd1ecd67a54c29c6e37d7f83a2ca2d331",
    ),
    (
        "docs/rust-rewrite/ci/gates.json",
        170743,
        "89aba2b481ed0eee5f4e7176c065a195c828b39eb42ce0ff85a7f5c978a0c4de",
    ),
];
fn image_operations(root: &Path) -> Result<()> {
    for &(path, size, sha) in OPS_INPUTS {
        material(root, path, size, sha)?;
    }
    let raw = material(root, OPS_TEST.0, OPS_TEST.2, OPS_TEST.3)?;
    prior(&OPS_TEST, &raw)?;
    Ok(())
}
const OPS_TEST: Successor = (
    "tests/test_deployment_artifacts.py",
    "a7ba1a26374d0111c89c3813c2f6e9754e5ff075",
    10048,
    "d3c44b8c75cfff929a19a2cb8b9ec83332a2133ae0fb77d8b7fc09f67478a3ba",
    12872,
    "e7634a07ab53f6409386a9515f46ab65ebccfb0747235d6ad43a07dcf54f431c",
    &[
        (
            "import re\nfrom pathlib import Path\nfrom urllib.error import HTTPError, URLError\n\nimport pytest\nimport yaml\n\nfrom scripts import promote_image_digest as promo\nfrom scripts import verify_docker_base_digest as verify\n\nROOT = Path(__file__).parents[1]\n\n",
            "import re\nfrom pathlib import Path\nfrom urllib.error import HTTPError\n\nimport pytest\nimport yaml\n\nfrom scripts import promote_image_digest as promo\n\nROOT = Path(__file__).parents[1]\n\n",
        ),
        (
            "    assert re.search(r\"^FROM python:[^@\\s]+@sha256:[0-9a-f]{64}$\", dockerfile, re.MULTILINE)\n\n    workflow = (ROOT / \".github/workflows/docker.yml\").read_text()\n    assert \"python3 scripts/verify_docker_base_digest.py\" in workflow\n\n\ndef test_pinned_base_parser_accepts_generic_python_tag() -> None:\n    digest = \"a\" * 64\n    assert verify._pinned_python_base(f\"FROM python:3.13-slim-bookworm@sha256:{digest}\\n\") == (\n        \"3.13-slim-bookworm\",\n        f\"sha256:{digest}\",\n    )\n\n\ndef test_pinned_base_parser_rejects_multiple_from_lines() -> None:\n    digest = \"a\" * 64\n    dockerfile = f\"FROM python:3.12-slim@sha256:{digest}\\nFROM scratch\\n\"\n\n    with pytest.raises(SystemExit, match=\"recognized\"):\n        verify._pinned_python_base(dockerfile)\n\n\n@pytest.mark.parametrize(\n    \"second_from\",\n    [\n        \"FROM --platform=linux/amd64 python:3.12-slim\",\n        \"FROM python:3.12-slim  # runtime stage\",\n    ],\n)\ndef test_pinned_base_parser_rejects_unrecognized_from_lines(second_from: str) -> None:\n    digest = \"a\" * 64\n    dockerfile = f\"FROM python:3.12-slim@sha256:{digest}\\n{second_from}\\n\"\n\n    with pytest.raises(SystemExit, match=\"recognized\"):\n        verify._pinned_python_base(dockerfile)\n\n\ndef test_registry_lookup_retries_transient_failures(monkeypatch: pytest.MonkeyPatch) -> None:\n    calls: list[object] = []\n\n    class Response:\n        def __enter__(self) -> \"Response\":\n            return self\n\n        def __exit__(self, *args: object) -> None:\n            return None\n\n    def fake_urlopen(request: object, timeout: int) -> Response:\n        calls.append((request, timeout))\n        if len(calls) < 3:\n            raise URLError(\"temporary failure\")\n        return Response()\n\n    monkeypatch.setattr(verify.urllib.request, \"urlopen\", fake_urlopen)\n    monkeypatch.setattr(verify.time, \"sleep\", lambda _seconds: None)\n\n    with verify._urlopen_with_retries(\"https://example.test\", label=\"test request\"):\n        pass\n\n    assert len(calls) == 3\n\n\ndef test_registry_lookup_classifies_permanent_http_failure(monkeypatch: pytest.MonkeyPatch) -> None:\n    calls: list[object] = []\n\n    def fake_urlopen(request: object, timeout: int) -> None:\n        calls.append((request, timeout))\n        raise HTTPError(\"https://example.test\", 404, \"not found\", {}, None)\n\n    monkeypatch.setattr(verify.urllib.request, \"urlopen\", fake_urlopen)\n\n    with pytest.raises(verify.RegistryLookupError, match=\"HTTP 404\"):\n        verify._urlopen_with_retries(\"https://example.test\", label=\"test request\")\n\n    assert len(calls) == 1\n\n\nGHCR_DIGEST_PIN = \"ghcr.io/magnus919/slopsearx@sha256:91194d146d205b1cf4688c1989da8f5f6b599a9627be23fd1ee7a4e488fda5b7\"\n",
            "    assert re.search(r\"^FROM python:[^@\\s]+@sha256:[0-9a-f]{64}$\", dockerfile, re.MULTILINE)\n\n    workflow = (ROOT / \".github/workflows/docker.yml\").read_text()\n    assert '\"$SSX_IMAGE_OPS_BIN\" verify-base-digests --dockerfile Dockerfile' in workflow\n\n\nGHCR_DIGEST_PIN = \"ghcr.io/magnus919/slopsearx@sha256:91194d146d205b1cf4688c1989da8f5f6b599a9627be23fd1ee7a4e488fda5b7\"\n",
        ),
        (
            "        step[\"run\"] for step in publish[\"steps\"] if step.get(\"name\") == \"Publish manifest from tested digests\"\n    )\n    assert '\"$(cat digests/amd64)\" \"$(cat digests/arm64)\"' in command\n    assert 'assert platforms == {(\"linux\", \"amd64\"), (\"linux\", \"arm64\")}' in command\n\n\ndef test_architecture_tags_follow_commit_retention_instead_of_each_run() -> None:\n",
            "        step[\"run\"] for step in publish[\"steps\"] if step.get(\"name\") == \"Publish manifest from tested digests\"\n    )\n    assert '\"$(cat digests/amd64)\" \"$(cat digests/arm64)\"' in command\n    assert \"verify-image-platforms --manifest manifest.json\" in command\n    assert \"python\" not in command\n\n\ndef test_architecture_tags_follow_commit_retention_instead_of_each_run() -> None:\n",
        ),
        (
            "    assert \"GITHUB_RUN_ATTEMPT\" not in tag_command\n\n\ndef test_health_builder_and_runtime_bases_are_both_verified(monkeypatch: pytest.MonkeyPatch) -> None:\n    dockerfile = (ROOT / \"Dockerfile\").read_text()\n    bases = verify.pinned_bases(dockerfile)\n    builder = dockerfile.split(\"# SlopSearX production image\", 1)[0]\n    assert \"COPY . .\" not in builder\n    assert [line for line in builder.splitlines() if line.startswith(\"COPY \")] == [\n        \"COPY Cargo.toml Cargo.lock rust-toolchain.toml ./\",\n        \"COPY crates/ crates/\",\n        \"COPY vendor/ vendor/\",\n    ]\n    assert [repository for repository, _, _ in bases] == [\"library/rust\", \"library/python\"]\n    assert bases[0][1] == \"1.99.0-bookworm\"\n    assert 'CMD [\"/usr/local/bin/ssx-healthcheck\"]' in dockerfile\n    assert \"healthcheck.py\" not in dockerfile\n    assert \"uvicorn\" in dockerfile  # This increment does not claim server retirement.\n    for malformed in [\n        dockerfile + \"\\nFROM scratch\\n\",\n        dockerfile.replace(\" AS healthcheck-builder\", \" AS arbitrary-builder\"),\n        dockerfile.replace(\"rust:1.99.0-bookworm@sha256:\", \"rust:1.99.0-bookworm@bad:\"),\n    ]:\n        with pytest.raises(SystemExit):\n            verify.pinned_bases(malformed)\n    calls: list[str] = []\n\n    def actual(repository: str, tag: str) -> str:\n        calls.append(repository)\n        return next(digest for repo, _, digest in bases if repo == repository)\n\n    monkeypatch.setattr(verify, \"registry_digest\", actual)\n    monkeypatch.setattr(\"sys.argv\", [\"verify_docker_base_digest.py\", \"--dockerfile\", str(ROOT / \"Dockerfile\")])\n    assert verify.main() == 0\n    assert calls == [\"library/rust\", \"library/python\"]\n    monkeypatch.setattr(verify, \"registry_digest\", lambda repository, tag: \"sha256:\" + \"0\" * 64)\n    assert verify.main() == 1\n",
            "    assert \"GITHUB_RUN_ATTEMPT\" not in tag_command\n\n\ndef test_native_health_builder_inputs_and_tool_handoff_remain_bounded() -> None:\n    dockerfile = (ROOT / \"Dockerfile\").read_text()\n    builder = dockerfile.split(\"# SlopSearX production image\", 1)[0]\n    assert [line for line in builder.splitlines() if line.startswith(\"COPY \")] == [\n        \"COPY Cargo.toml Cargo.lock rust-toolchain.toml ./\",\n        \"COPY crates/ crates/\",\n        \"COPY vendor/ vendor/\",\n    ]\n    assert 'CMD [\"/usr/local/bin/ssx-healthcheck\"]' in dockerfile\n    assert \"uvicorn\" in dockerfile\n    workflow = yaml.safe_load((ROOT / \".github/workflows/docker.yml\").read_text())\n    jobs = workflow[\"jobs\"]\n    handoff = next(\n        step for step in jobs[\"build-and-push\"][\"steps\"] if step.get(\"name\") == \"Hand off native manifest verifier\"\n    )\n    assert handoff[\"if\"] == \"github.event_name == 'push' && matrix.arch == 'amd64'\"\n    download = next(\n        step\n        for step in jobs[\"publish-manifest\"][\"steps\"]\n        if step.get(\"name\") == \"Download native manifest verifier from this run\"\n    )\n    assert handoff[\"with\"][\"name\"] == download[\"with\"][\"name\"]\n    assert \"run-id\" not in download[\"with\"]\n    assert \"repository\" not in download[\"with\"]\n    assert not (ROOT / \"scripts/verify_docker_base_digest.py\").exists()\n",
        ),
    ],
);
fn python_successor(root: &Path, row: &Successor) -> Result<Vec<u8>> {
    need(
        row.0 != VERIFIER,
        "retired verifier has no physical current operand",
    )?;
    need(row.0 == OPS_TEST.0, "closed current Python successor")?;
    let raw = material(root, OPS_TEST.0, OPS_TEST.2, OPS_TEST.3)?;
    prior(&OPS_TEST, &raw)
}
