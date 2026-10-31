//! Verify a completed actual workspace build superset, not optional lock membership.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};
type Result<T> = std::result::Result<T, String>;
const FORBIDDEN: [&str; 6] = [
    "cpython",
    "pyo3",
    "pyo3-ffi",
    "pyo3-macros",
    "pythonize",
    "rust-cpython",
];
fn need(ok: bool, why: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(why.into()) }
}
fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn package_name(id: &str) -> Result<&str> {
    let (source, fragment) = id
        .rsplit_once('#')
        .ok_or("Cargo package identity missing fragment")?;
    let name = if let Some((name, version)) = fragment.split_once('@') {
        need(!version.is_empty(), "Cargo package version missing")?;
        name
    } else {
        need(
            source.starts_with("path+file://")
                && fragment.starts_with(|c: char| c.is_ascii_digit()),
            "unsupported Cargo package identity",
        )?;
        source
            .rsplit('/')
            .next()
            .ok_or("Cargo path package missing name")?
    };
    need(
        !name.is_empty()
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
        "invalid Cargo package name",
    )?;
    Ok(name)
}
fn parse(raw: &[u8], producer_exit: &[u8]) -> Result<Vec<Value>> {
    need(
        producer_exit == b"0\n",
        "actual Cargo producer did not exit zero",
    )?;
    let raw = std::str::from_utf8(raw).map_err(|_| "Cargo stream is not UTF8")?;
    let rows = raw
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).map_err(|_| "malformed Cargo JSON record".into()))
        .collect::<Result<Vec<Value>>>()?;
    need(rows.len() >= 2, "empty or incomplete Cargo stream")?;
    need(
        rows.last()
            .is_some_and(|v| v["reason"] == "build-finished" && v["success"] == true),
        "Cargo completion absent or failed",
    )?;
    for row in &rows[..rows.len() - 1] {
        need(
            matches!(
                row["reason"].as_str(),
                Some("compiler-artifact" | "compiler-message" | "build-script-executed")
            ),
            "unexpected or premature Cargo terminator",
        )?;
        let id = row["package_id"]
            .as_str()
            .ok_or("Cargo record package identity missing")?;
        need(
            !FORBIDDEN.contains(&package_name(id)?),
            "compiled Python bridge package refused",
        )?;
        if row["reason"] == "compiler-artifact" {
            let strings = |value: &Value| {
                value
                    .as_array()
                    .is_some_and(|items| items.iter().all(Value::is_string))
            };
            need(
                row["manifest_path"].is_string()
                    && row["target"]["name"].is_string()
                    && row["target"]["src_path"].is_string()
                    && strings(&row["target"]["kind"])
                    && strings(&row["target"]["crate_types"])
                    && strings(&row["filenames"])
                    && strings(&row["features"])
                    && row["profile"]["test"].is_boolean()
                    && row["fresh"].is_boolean(),
                "malformed Cargo artifact role",
            )?;
        }
    }
    need(
        rows.iter().any(|v| v["reason"] == "compiler-artifact"),
        "Cargo artifacts missing",
    )?;
    Ok(rows)
}
fn source_directory(id: &str) -> Result<PathBuf> {
    let (source, _) = id.rsplit_once('#').ok_or("server source package missing")?;
    let uri = source
        .strip_prefix("path+file://")
        .ok_or("server is not a local source package")?;
    let mut decoded = Vec::new();
    let bytes = uri.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%' {
            let pair = bytes
                .get(at + 1..at + 3)
                .ok_or("short Cargo source escape")?;
            let pair = std::str::from_utf8(pair).map_err(|_| "invalid Cargo source escape")?;
            decoded.push(u8::from_str_radix(pair, 16).map_err(|_| "invalid Cargo source escape")?);
            at += 3;
        } else {
            decoded.push(bytes[at]);
            at += 1;
        }
    }
    Ok(PathBuf::from(
        String::from_utf8(decoded).map_err(|_| "Cargo source path is not UTF8")?,
    ))
}
fn server<'a>(
    rows: &'a [Value],
    root: &Path,
    target_root: &Path,
    triple: &str,
    image: &Path,
) -> Result<&'a Value> {
    need(
        !triple.is_empty()
            && triple
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
        "invalid native target triple",
    )?;
    let expected = target_root.join(triple).join("debug/ssx-server");
    need(
        image == expected,
        "server image is not the requested native target role",
    )?;
    let candidates: Vec<_> = rows
        .iter()
        .filter(|v| v["reason"] == "compiler-artifact" && v["target"]["name"] == "ssx-server")
        .collect();
    need(
        candidates.len() == 1,
        "missing or ambiguous server artifact",
    )?;
    let row = candidates[0];
    need(
        package_name(row["package_id"].as_str().ok_or("server package missing")?)? == "ssx-server",
        "server package identity differs",
    )?;
    need(
        source_directory(row["package_id"].as_str().ok_or("server package missing")?)?
            == root.join("crates/ssx-server"),
        "server package source root differs",
    )?;
    need(
        row["manifest_path"].as_str().map(Path::new)
            == Some(root.join("crates/ssx-server/Cargo.toml").as_path())
            && row["target"]["src_path"].as_str().map(Path::new)
                == Some(root.join("crates/ssx-server/src/main.rs").as_path()),
        "server source role differs",
    )?;
    need(
        row["target"]["kind"] == json!(["bin"])
            && row["target"]["crate_types"] == json!(["bin"])
            && row["profile"]["test"] == false
            && row["features"] == json!([]),
        "server target/profile/features differ",
    )?;
    need(row["fresh"].is_boolean(), "Cargo freshness field missing")?;
    need(
        row["executable"].as_str().map(Path::new) == Some(image)
            && row["filenames"]
                .as_array()
                .is_some_and(|v| v.iter().any(|n| n.as_str().map(Path::new) == Some(image))),
        "server executable filename differs",
    )?;
    Ok(row)
}
fn regular(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let m = fs::symlink_metadata(path).map_err(|_| "artifact evidence missing")?;
    need(
        m.is_file() && !m.file_type().is_symlink() && m.len() <= limit,
        "artifact evidence is nonregular or over bound",
    )?;
    let mut file = fs::File::open(path).map_err(|_| "artifact evidence unreadable")?;
    let opened = file
        .metadata()
        .map_err(|_| "artifact evidence metadata unreadable")?;
    need(
        opened.is_file() && opened.len() == m.len(),
        "artifact evidence changed before read",
    )?;
    let mut raw = Vec::new();
    file.by_ref()
        .take(
            limit
                .checked_add(1)
                .ok_or("artifact evidence limit overflow")?,
        )
        .read_to_end(&mut raw)
        .map_err(|_| "artifact evidence read failed")?;
    need(
        raw.len() as u64 <= limit
            && raw.len() as u64 == opened.len()
            && file
                .metadata()
                .map_err(|_| "artifact evidence metadata unreadable")?
                .len()
                == opened.len(),
        "artifact evidence length changed or exceeded bound",
    )?;
    Ok(raw)
}
fn image_identity(path: &Path) -> Result<(u64, String)> {
    let metadata = fs::symlink_metadata(path).map_err(|_| "server image missing")?;
    need(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "server image is nonregular",
    )?;
    need(
        metadata.len() <= 2 * 1024 * 1024 * 1024,
        "server image over bound",
    )?;
    let mut file = fs::File::open(path).map_err(|_| "server image unreadable")?;
    let mut hash = Sha256::new();
    let mut length = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        let n = file
            .read(&mut buffer)
            .map_err(|_| "server image read failed")?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
        length += n as u64;
        need(length <= metadata.len(), "server image grew during read")?;
    }
    need(length == metadata.len(), "server image length changed")?;
    Ok((
        length,
        hash.finalize().iter().map(|b| format!("{b:02x}")).collect(),
    ))
}
fn same_receipt(expected: &Value, observed: &Value) -> Result<()> {
    need(
        expected == observed,
        "captured build boundary differs from current source/stream/image",
    )
}
fn run(args: &[String]) -> Result<Value> {
    need(
        args.len() == 7,
        "usage: foundation_python_boundary ROOT TARGET_ROOT TRIPLE STREAM EXIT_FILE SERVER CAPTURE_RECEIPT|--record",
    )?;
    let root = Path::new(&args[0])
        .canonicalize()
        .map_err(|_| "source root missing")?;
    let target = Path::new(&args[1])
        .canonicalize()
        .map_err(|_| "target root missing")?;
    let image = PathBuf::from(&args[5]);
    let image = if image.is_absolute() {
        image
    } else {
        root.join(image)
    };
    let before = image_identity(&image)?;
    need(
        image.canonicalize().map_err(|_| "server path missing")? == image
            && image.starts_with(&target),
        "server image path is not canonical target leaf",
    )?;
    let stream = regular(Path::new(&args[3]), 128 * 1024 * 1024)?;
    let exit = regular(Path::new(&args[4]), 16)?;
    let rows = parse(&stream, &exit)?;
    let artifact = server(&rows, &root, &target, &args[2], &image)?;
    need(
        image_identity(&image)? == before,
        "server image changed during boundary audit",
    )?;
    let observed = json!({"schema":1,"scope":"complete actual workspace compiler-artifact superset; selected normal native server image; no optional-lock membership inference or OS launch-image claim","source_root":root,"target_root":target,"target":args[2],"producer_exit":0,"stream_bytes":stream.len(),"stream_sha256":digest(&stream),"compiler_artifacts":rows.iter().filter(|v|v["reason"]=="compiler-artifact").count(),"all_package_records_audited":rows.len()-1,"forbidden_names":FORBIDDEN,"server_artifact":artifact,"server_image":{"path":image,"bytes":before.0,"sha256":before.1}});
    if args[6] != "--record" {
        let expected: Value = serde_json::from_slice(&regular(Path::new(&args[6]), 1048576)?)
            .map_err(|_| "captured boundary JSON malformed")?;
        same_receipt(&expected, &observed)?;
    }
    Ok(observed)
}
fn main() {
    match run(&std::env::args().skip(1).collect::<Vec<_>>()) {
        Ok(value) => println!("{}", serde_json::to_string(&value).expect("JSON value")),
        Err(error) => {
            eprintln!("rust foundation Python boundary: {error}");
            std::process::exit(1)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (PathBuf, PathBuf, PathBuf, Vec<Value>) {
        let root = std::env::temp_dir().join("ssx-boundary-test-source");
        let target = root.join("target");
        let image = target.join("aarch64-unknown-linux-gnu/debug/ssx-server");
        let row = json!({"reason":"compiler-artifact","package_id":format!("path+file://{}/crates/ssx-server#0.1.0",root.display()),"manifest_path":root.join("crates/ssx-server/Cargo.toml"),"target":{"name":"ssx-server","kind":["bin"],"crate_types":["bin"],"src_path":root.join("crates/ssx-server/src/main.rs")},"profile":{"test":false},"features":[],"filenames":[image],"executable":image,"fresh":true});
        (
            root,
            target,
            image,
            vec![row, json!({"reason":"build-finished","success":true})],
        )
    }
    fn bytes(rows: &[Value]) -> Vec<u8> {
        rows.iter()
            .map(|v| format!("{v}\n"))
            .collect::<String>()
            .into_bytes()
    }
    #[test]
    fn actual_superset_inactive_lock_and_fresh_records_are_distinct() {
        let (root, target, image, rows) = fixture();
        // A dormant lock package is not a producer record; jiter itself is allowed.
        let mut actual = rows.clone();
        let mut jiter = rows[0].clone();
        jiter["package_id"] = json!("registry+https://example.invalid/index#jiter@0.14.0");
        jiter["target"]["name"] = json!("jiter");
        jiter["target"]["kind"] = json!(["lib"]);
        jiter["target"]["crate_types"] = json!(["lib"]);
        jiter["executable"] = Value::Null;
        actual.insert(0, jiter);
        let parsed = parse(&bytes(&actual), b"0\n").unwrap();
        server(&parsed, &root, &target, "aarch64-unknown-linux-gnu", &image).unwrap();
        actual[0]["fresh"] = json!(false);
        assert!(parse(&bytes(&actual), b"0\n").is_ok());
        for name in FORBIDDEN {
            for kind in ["lib", "proc-macro", "custom-build"] {
                for fresh in [true, false] {
                    let mut bad = actual.clone();
                    bad[0]["package_id"] = json!(format!(
                        "registry+https://example.invalid/index#{name}@0.28.2"
                    ));
                    bad[0]["target"]["kind"] = json!([kind]);
                    bad[0]["fresh"] = json!(fresh);
                    assert_eq!(
                        parse(&bytes(&bad), b"0\n").unwrap_err(),
                        "compiled Python bridge package refused"
                    );
                }
            }
        }
        assert_eq!(
            package_name("path+file:///owned/pyo3#0.28.2").unwrap(),
            "pyo3"
        );
        assert_eq!(
            package_name("path+file:///owned/renamed#pyo3@0.28.2").unwrap(),
            "pyo3"
        );
        assert_eq!(
            source_directory("path+file:///owned/space%20and%20caf%C3%A9#ssx-server@0.1.0")
                .unwrap(),
            PathBuf::from("/owned/space and café")
        );
        assert!(package_name("unsupported").is_err());
        let observed = json!({"server_image":{"sha256":"actual","bytes":12},"target":"native","stream_sha256":"stream"});
        same_receipt(&observed, &observed).unwrap();
        for pointer in ["/server_image/sha256", "/target", "/stream_sha256"] {
            let mut changed = observed.clone();
            *changed.pointer_mut(pointer).unwrap() = json!("changed");
            assert!(same_receipt(&changed, &observed).is_err());
        }
    }
    #[test]
    fn producer_completion_and_unique_server_role_fail_closed() {
        let (root, target, image, rows) = fixture();
        let good = bytes(&rows);
        assert!(parse(&good, b"1\n").is_err());
        assert!(parse(b"", b"0\n").is_err());
        assert!(parse(b"{", b"0\n").is_err());
        let mut malformed = rows.clone();
        malformed[0].as_object_mut().unwrap().remove("fresh");
        assert!(parse(&bytes(&malformed), b"0\n").is_err());
        assert!(parse(&bytes(&rows[..1]), b"0\n").is_err());
        let mut bad = rows.clone();
        bad.last_mut().unwrap()["success"] = json!(false);
        assert!(parse(&bytes(&bad), b"0\n").is_err());
        let mut bad = rows.clone();
        bad.push(rows[0].clone());
        assert!(parse(&bytes(&bad), b"0\n").is_err());
        let mut bad = rows.clone();
        bad.insert(0, rows[0].clone());
        assert!(server(&bad, &root, &target, "aarch64-unknown-linux-gnu", &image).is_err());
        assert!(
            server(
                &rows[1..],
                &root,
                &target,
                "aarch64-unknown-linux-gnu",
                &image
            )
            .is_err()
        );
        for (pointer, value) in [
            (
                "/package_id",
                json!("registry+https://example.invalid/index#ssx-server@0.1.0"),
            ),
            ("/profile/test", json!(true)),
            ("/features", json!(["unreviewed"])),
            ("/target/kind", json!(["test"])),
            ("/manifest_path", json!("/wrong/Cargo.toml")),
            ("/target/src_path", json!("/wrong/main.rs")),
            ("/executable", json!("/wrong/ssx-server")),
        ] {
            let mut bad = rows.clone();
            *bad[0].pointer_mut(pointer).unwrap() = value;
            assert!(server(&bad, &root, &target, "aarch64-unknown-linux-gnu", &image).is_err());
        }
        assert!(server(&rows, &root, &target, "x86_64-unknown-linux-gnu", &image).is_err());
        assert!(
            server(
                &rows,
                &root,
                &target,
                "aarch64-unknown-linux-gnu",
                &target.join("wrong")
            )
            .is_err()
        );
    }
}
