//! Exact D main successor over accepted A06. Shared by all native source consumers.
use sha2::{Digest, Sha256};
type Result<T> = std::result::Result<T, String>;
pub const PRIOR_SHA: &str = "e695b32f8617ff35ca939b4e8f8f00831ca3465319ba3961680574db1da8a1f2";
pub const CURRENT_SHA: &str = "cb11fa4fc7bbaf969f7fd99f1e8bcd270c8939e3e37eaec8b47906b0d943c0df";
const MAIN_SCOPE: &str = "    } else if id == \"W07\" {\n        \"exact native receipt/record/lineage/composition/MCP execution and measured frozen Python wire subset; no providers, production activation, startup or complete rewrite certification\"\n";
fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
/// Remove only three unique D insertions, then prove every prior byte. No hash repin.
pub fn reconstruct(current: &[u8]) -> Result<Vec<u8>> {
    if sha(current) == PRIOR_SHA {
        return Ok(current.to_vec());
    }
    if sha(current) != CURRENT_SHA {
        return Err("W07 main successor is not the exact reviewed afterimage".into());
    }
    let mut text = String::from_utf8(current.to_vec()).map_err(|_| "W07 main not UTF8")?;
    for insertion in [
        "mod w07;\n",
        "        \"W07\" => w07::check(root, records),\n",
        MAIN_SCOPE,
    ] {
        if text.matches(insertion).count() != 1 {
            return Err("W07 main successor insertion changed/missing/duplicate".into());
        }
        text = text.replacen(insertion, "", 1);
    }
    if sha(text.as_bytes()) != PRIOR_SHA {
        return Err("W07 main successor changed unrelated prior bytes".into());
    }
    Ok(text.into_bytes())
}
