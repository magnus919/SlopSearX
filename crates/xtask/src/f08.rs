//! Source-bound native transport qualification; no provider traffic.
use crate::{Result, command};
use serde_json::{Value, json};
use std::{collections::BTreeSet, path::Path};
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let coverage = crate::a08::header_successor::verify(root, records)?;
    if coverage["schema_version"] != 1
        || coverage["plan_id"] != "F08"
        || coverage["baseline"] != "7cc420f7ffb61127750e77bf6130e225925f9186"
    {
        return Err("invalid F08 coverage identity".into());
    }
    let required: BTreeSet<_> = [
        "slopsearx/adapter.py",
        "slopsearx/proxypool.py",
        "tests/test_http_pooling.py",
        "tests/test_duckduckgo_resilience.py",
        "engines/pubmed.py",
        "engines/internetarchive.py",
    ]
    .into_iter()
    .collect();
    let sources = coverage["sources"]
        .as_array()
        .ok_or("missing F08 sources")?;
    let paths: BTreeSet<_> = sources.iter().filter_map(|s| s["path"].as_str()).collect();
    if sources.len() != required.len() || paths != required {
        return Err("F08 source inventory differs".into());
    }
    for source in sources {
        let path = source["path"].as_str().ok_or("missing F08 source path")?;
        if !path.starts_with("slopsearx/")
            && !path.starts_with("engines/")
            && !path.starts_with("tests/")
        {
            return Err("invalid F08 source path".into());
        }
        let actual = command(root, "git", &["hash-object", "--", path], records)?;
        if source["git_blob"] != actual.trim() {
            return Err(format!("F08 source drift: {path}"));
        }
    }
    let listed = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-infra",
            "--test",
            "http",
            "--",
            "--list",
        ],
        records,
    )?;
    let actual: BTreeSet<_> = listed
        .lines()
        .filter_map(|s| s.strip_suffix(": test"))
        .map(str::to_owned)
        .collect();
    let expected: BTreeSet<_> = coverage["tests"]
        .as_array()
        .ok_or("missing F08 tests")?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or("invalid F08 test".to_string())
        })
        .collect::<Result<_>>()?;
    if expected.len() != 25 || actual != expected {
        return Err("F08 test inventory differs from reviewed coverage".into());
    }
    command(
        root,
        "cargo",
        &["test", "--locked", "-p", "ssx-infra", "--test", "http"],
        records,
    )?;
    let listed = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-infra",
            "--lib",
            "http::decode_tests",
            "--",
            "--list",
        ],
        records,
    )?;
    let actual: BTreeSet<_> = listed
        .lines()
        .filter_map(|s| s.strip_suffix(": test"))
        .map(str::to_owned)
        .collect();
    let expected: BTreeSet<_> = coverage["unit_tests"]
        .as_array()
        .ok_or("missing F08 worker tests")?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or("invalid F08 worker test".to_string())
        })
        .collect::<Result<_>>()?;
    if expected.len() != 4 || actual != expected {
        return Err("F08 worker ownership test inventory differs".into());
    }
    command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-infra",
            "--lib",
            "http::decode_tests",
        ],
        records,
    )?;
    records.push(json!({"contract":"F08","coverage":"docs/rust-rewrite/f08/coverage.json","scope":"synthetic loopback, TLS, environment subprocess and offline exchanges; no provider parser or live parity claim","status":"passed"}));
    Ok(())
}
