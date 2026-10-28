//! Closed normal-merge composition of the reviewed W07 and T02 source parents.
//! Physical joint identity precedes either historical parent interpretation.
use sha2::{Digest, Sha256};
use std::{fs, path::Path, process::Command};
#[path = "../../snapshot-fixture-amendment/main-custody.rs"]
mod snapshot_fixture_main;
type Result<T> = std::result::Result<T, String>;
#[derive(Clone, Copy)]
pub(crate) enum Parent {
    W07,
    T02,
}
pub(crate) const MAIN_SHA: &str =
    "abc9435b419cfa6f0013fe1177d27b6661437baabe40d3824fa95185e6da40ee";
pub(crate) const LOCK_SHA: &str =
    "61be734ee8259d76108383154509e8c261e2cf064dc9ebf4def23a9d2a8332dc";
struct Prior {
    commit: &'static str,
    sha: &'static str,
    bytes: usize,
    blob: &'static str,
    edits: &'static [(usize, &'static str, &'static str)],
}
struct Row {
    path: &'static str,
    sha: &'static str,
    bytes: usize,
    w07: Prior,
    t02: Prior,
}
const ROWS: &[Row] = &[
    Row {
        path: "crates/xtask/src/main.rs",
        sha: "abc9435b419cfa6f0013fe1177d27b6661437baabe40d3824fa95185e6da40ee",
        bytes: 39997,
        w07: Prior {
            commit: "88cd243140b6a4200141e04361e6342b80008a80",
            sha: "cb11fa4fc7bbaf969f7fd99f1e8bcd270c8939e3e37eaec8b47906b0d943c0df",
            bytes: 39944,
            blob: "2596273de93c71e914b435c861ebee5c29510cb6",
            edits: &[
                (404, "mod t02;\n", ""),
                (18028, "        \"T02\" => t02::check(root, records),\n", ""),
                (
                    37424,
                    "            execute(&root, \"T03\", &mut vec![])\n",
                    "            execute(&root, \"T02\", &mut vec![])\n",
                ),
            ],
        },
        t02: Prior {
            commit: "b79a5a9c3ed78aaf42a38da61454717c95388920",
            sha: "8755129fbac7454a1ae25982dc44ce1cb92b8ead7ecf650a3901aa2f08d4a23c",
            bytes: 39725,
            blob: "a98daca3091721108dd4b53f8e519e34a5712e6e",
            edits: &[
                (538, "mod w07;\n", ""),
                (18688, "        \"W07\" => w07::check(root, records),\n", ""),
                (
                    27525,
                    "    } else if id == \"W07\" {\n        \"exact native receipt/record/lineage/composition/MCP execution and measured frozen Python wire subset; no providers, production activation, startup or complete rewrite certification\"\n",
                    "",
                ),
            ],
        },
    },
    Row {
        path: "Cargo.lock",
        sha: "61be734ee8259d76108383154509e8c261e2cf064dc9ebf4def23a9d2a8332dc",
        bytes: 87572,
        w07: Prior {
            commit: "88cd243140b6a4200141e04361e6342b80008a80",
            sha: "9b2ab4a8dfb4a6c41c0af3575a6f77c0bbaf139f7efe83f40e60b9380974fd58",
            bytes: 87538,
            blob: "1ea7c32f8337b33135cecc1274ca55c3ec69cd43",
            edits: &[
                (61783, " \"serde-saphyr\",\n", ""),
                (61901, " \"ssx-registry\",\n", ""),
            ],
        },
        t02: Prior {
            commit: "b79a5a9c3ed78aaf42a38da61454717c95388920",
            sha: "96e9a7760d163fc5ea2afa23033e58cfb7e59d0f2d93f66d8847ef352e0b765d",
            bytes: 87558,
            blob: "ec42b15cc6bd863230688af502921fd3f0947778",
            edits: &[(62649, " \"aws-lc-rs\",\n", "")],
        },
    },
    Row {
        path: "crates/ssx-infra/src/search.rs",
        sha: "b74fc961b64cd2faced084c4894e8b4c5cd481642bc20fd9192ce7a37bcf0bbb",
        bytes: 90389,
        w07: Prior {
            commit: "88cd243140b6a4200141e04361e6342b80008a80",
            sha: "23989888f2434630b9185d339f8548a8e73e8e08846ce00419609be4b439c05c",
            bytes: 89900,
            blob: "56fe1c52f543728d4526c8596a5339dc3c1a0f76",
            edits: &[
                (
                    15256,
                    "        let service = SearchService(inner);\n        let advisories =\n            service.read_advisories(&self.request, &self.scope, self.full_result_count())?;\n        Ok(SearchReadView {\n            response: self.present(),\n            advisories,\n        })\n    }\n}\nimpl SearchService {\n    /// Read-only public notes. This does not acquire scope, quota, cache or dispatch authority.\n    /// HTTP uses the same writer for a policy-checked empty eligible scope.\n    pub fn read_advisories(\n        &self,\n        request: &SearchRequest,\n        scope: &ScopeDecision,\n        full_result_count: usize,\n    ) -> Result<Vec<ssx_json::Value>, SearchError> {\n        let policy = self.current_policy(self.0.deps.clock.capture().0)?;\n        let query = request.query.to_lowercase();\n",
                    "        let service = SearchService(inner.clone());\n        let policy = service.current_policy(inner.deps.clock.capture().0)?;\n        let query = self.request.query.to_lowercase();\n",
                ),
                (
                    16422,
                    "        let catalog = self\n            .0\n",
                    "        let catalog = inner\n",
                ),
                (
                    16720,
                    "                let descriptor = self.0.registry.descriptor(&e.name)?;\n",
                    "                let descriptor = inner.registry.descriptor(&e.name)?;\n",
                ),
                (
                    17707,
                    "            routing_configured: self.0.deps.jev.is_some(),\n            reranking_configured: self.0.deps.reranker.is_some(),\n",
                    "            routing_configured: inner.deps.jev.is_some(),\n            reranking_configured: inner.deps.reranker.is_some(),\n",
                ),
                (
                    17843,
                    "        Ok(ssx_application::advisories::search(\n            request,\n            scope,\n            &context,\n            full_result_count,\n        ))\n",
                    "        Ok(SearchReadView {\n            response: self.present(),\n            advisories: ssx_application::advisories::search(\n                &self.request,\n                &self.scope,\n                &context,\n                self.full_result_count(),\n            ),\n        })\n",
                ),
                (18003, "\n", ""),
            ],
        },
        t02: Prior {
            commit: "b79a5a9c3ed78aaf42a38da61454717c95388920",
            sha: "a1b310e2ef8692c96109bfd780c0e37d7e50692231e11eceb48e05860e0b5927",
            bytes: 86610,
            blob: "f4ad63519cc8d62136dda49db3ad8087c6f7b5ea",
            edits: &[
                (
                    10435,
                    "    /// Observe the caller's existing signal without creating an owner or task.\n    pub async fn cancelled(&self) {\n        cancelled(&mut self.0.subscribe()).await;\n    }\n",
                    "",
                ),
                (
                    11122,
                    "/// Trusted, bounded, in-memory current policy. Never caller input or I/O.\npub trait CurrentPolicy: Send + Sync {\n    fn current(&self) -> PolicyService;\n}\n",
                    "",
                ),
                (
                    11293,
                    "    policy_source: Option<Arc<dyn CurrentPolicy>>,\n",
                    "",
                ),
                (
                    18025,
                    "    /// Install only before sharing, with the exact initial dependency policy.\n    /// Dynamic consumers still use the existing shared policy methods.\n    pub fn with_policy_source(\n        mut self,\n        source: Arc<dyn CurrentPolicy>,\n    ) -> Result<Self, SearchError> {\n        let inner = Arc::get_mut(&mut self.0).ok_or(SearchError::InvalidConfiguration)?;\n        if format!(\"{:?}\", source.current()) != format!(\"{:?}\", inner.deps.policy) {\n            return Err(SearchError::InvalidConfiguration);\n        }\n        inner.policy_source = Some(source);\n        Ok(self)\n    }\n",
                    "",
                ),
                (20994, "            policy_source: None,\n", ""),
                (
                    21845,
                    "        let mut policy = self\n            .0\n            .policy_source\n            .as_ref()\n            .map_or_else(|| self.0.deps.policy.clone(), |source| source.current());\n",
                    "        let mut policy = self.0.deps.policy.clone();\n",
                ),
                (
                    82194,
                    "    #[test]\n    fn current_policy_source_is_construction_only_and_preserves_default_generation() {\n        struct Current(std::sync::RwLock<PolicyService>);\n        impl CurrentPolicy for Current {\n            fn current(&self) -> PolicyService {\n                self.0.read().unwrap().clone()\n            }\n        }\n        let policy = PolicyService {\n            policy: SearchPolicy::default(),\n            catalog: CatalogSnapshot::new(vec![]).unwrap(),\n            resolver: ScopeResolver::default(),\n        };\n        let current = Arc::new(Current(std::sync::RwLock::new(policy.clone())));\n        let service = SearchService::new(\n            SearchConfig::default(),\n            SearchDependencies {\n                policy: policy.clone(),\n                engines: BTreeMap::new(),\n                routing_identity: RoutingIdentity {\n                    catalog: None,\n                    sensitive: Default::default(),\n                    tier1: Default::default(),\n                    budget: None,\n                    ranking_strategy: \"presence\".into(),\n                    work_group_version: \"scholarly-work-v3\".into(),\n                    rerank_identity: None,\n                    routing_extension: None,\n                },\n                cache: None,\n                client_admission: None,\n                suggestions: None,\n                jev: None,\n                routing_cache: None,\n                metrics: Arc::new(NoopSink),\n                supervisor: Arc::new(NoopSink),\n                audit: Arc::new(NoopSink),\n                cooldowns: None,\n                owned_http: vec![],\n                owned_stores: vec![],\n                clock: Arc::new(SystemSearchClock),\n                reranker: None,\n            },\n        )\n        .unwrap();\n        assert_eq!(\n            format!(\n                \"{:?}\",\n                service\n                    .current_policy(WallClockTimestamp::new(1.0).unwrap())\n                    .unwrap()\n            ),\n            format!(\"{policy:?}\")\n        );\n        let service = service.with_policy_source(current.clone()).unwrap();\n        current.0.write().unwrap().policy.sensitive_allowed = true;\n        assert!(\n            service\n                .current_policy(WallClockTimestamp::new(1.0).unwrap())\n                .unwrap()\n                .policy\n                .sensitive_allowed\n        );\n        assert!(!service.0.deps.policy.policy.sensitive_allowed);\n        let clone = service.clone();\n        assert!(matches!(\n            service.with_policy_source(current),\n            Err(SearchError::InvalidConfiguration)\n        ));\n        drop(clone);\n    }\n\n",
                    "",
                ),
            ],
        },
    },
    Row {
        path: "crates/xtask/src/a13.rs",
        sha: "681d6f0d1f1ec6af77bb88aa1774aa8aa8669f390dd801bf7391308db7494ff0",
        bytes: 57322,
        w07: Prior {
            commit: "88cd243140b6a4200141e04361e6342b80008a80",
            sha: "cbaabdef5996afb5300cf7b13c620eac94be6fd71babb19abf184c595e3e6a6c",
            bytes: 57322,
            blob: "2afb1a6182eaa3ba9ca5712bb32a3e410cc7f9f1",
            edits: &[(
                454,
                "const QUALIFICATION_SHA: &str = \"77661b6d69dc2926a2d01d277e68e22932d7fb1aae258fb272ab1192dff18c6e\";\n",
                "const QUALIFICATION_SHA: &str = \"e2d9249fbadd0eca06a10215f796defc5845722c63fcf9f409ab9eac8a85f74e\";\n",
            )],
        },
        t02: Prior {
            commit: "ea947918335f812c67c3b3e47a23ce4ede2a2d30",
            sha: "7b8c213676b77a5511da43bb1f8bb3d889d4964e3ffceb4719b21801cd5fea95",
            bytes: 56700,
            blob: "31f03497b0bf9014907a004deaaae84858d9a2ce",
            edits: &[
                (
                    34330,
                    "        names.push(a06_source_successor::w07_evidence_successor::TEST.into());\n",
                    "",
                ),
                (
                    49740,
                    "        a06_source_successor::w07_evidence_successor::controls(&root).unwrap();\n",
                    "",
                ),
                (
                    54878,
                    "        let w07 = a06_source_successor::w07_evidence_successor::TEST;\n        assert_eq!(count(w07), 1);\n        let mut omitted_w07 = exact.clone();\n        omitted_w07.retain(|name| name != w07);\n        assert!(same(&omitted_w07, &exact, \"W07 missing additive identity\").is_err());\n        let mut duplicate_w07 = exact.clone();\n        duplicate_w07.push(w07.into());\n        assert!(same(&duplicate_w07, &exact, \"W07 duplicate additive identity\").is_err());\n",
                    "",
                ),
            ],
        },
    },
    Row {
        path: "docs/rust-rewrite/a13/source-amendments/validator.rs",
        sha: "9e58e8da346f242483915dda3ecbf369371adf665f178eeb301a8a0b1ba62cc1",
        bytes: 51220,
        w07: Prior {
            commit: "88cd243140b6a4200141e04361e6342b80008a80",
            sha: "a0752936363e5b3053f2e96dc04eac746b6f955f1bf40a14a34671a5f2120bfe",
            bytes: 50431,
            blob: "19d4a48b10815595d9af22490c77eda1d26c2cbc",
            edits: &[
                (
                    14307,
                    "",
                    "    let container = fs::read(root.join(container_path)).map_err(|error| error.to_string())?;\n",
                ),
                (
                    14433,
                    "    let container = causal_successor::a06_source_successor::historical_caller(\n        root,\n        container_path,\n        container_sha,\n    )?;\n",
                    "",
                ),
                (
                    34114,
                    "        let container = causal_successor::a06_source_successor::historical_caller(\n            root,\n            cp,\n            pin[\"container_sha256\"]\n                .as_str()\n                .ok_or(\"A13 lock caller hash absent\")?,\n        )?;\n",
                    "        let container_path = root.join(cp);\n        let container = fs::read(&container_path).map_err(|e| e.to_string())?;\n",
                ),
                (
                    37611,
                    "    let container = causal_successor::a06_source_successor::historical_caller(\n        root,\n        cp,\n        pin[\"container_sha256\"]\n            .as_str()\n            .ok_or(\"A13 third-package caller hash absent\")?,\n    )?;\n",
                    "    let container = fs::read(root.join(cp)).map_err(|e| e.to_string())?;\n",
                ),
                (
                    40406,
                    "            let container = causal_successor::a06_source_successor::historical_caller(\n                root,\n                container_path,\n                pin[\"container_sha256\"]\n                    .as_str()\n                    .ok_or(\"A13 additive caller hash absent\")?,\n            )?;\n",
                    "            let container = fs::read(root.join(container_path)).map_err(|e| e.to_string())?;\n",
                ),
                (
                    41766,
                    "                causal_successor::a06_source_successor::generation_control_operand(\n                    root,\n                    path,\n                    &entry[\"successor\"],\n                    &actual_current,\n                )?\n",
                    "                actual_current\n",
                ),
                (
                    46202,
                    "    let container = causal_successor::a06_source_successor::historical_caller(\n        root,\n        container_path,\n        container_sha,\n    )?;\n",
                    "    let container = fs::read(root.join(container_path)).map_err(|error| error.to_string())?;\n",
                ),
            ],
        },
        t02: Prior {
            commit: "ea947918335f812c67c3b3e47a23ce4ede2a2d30",
            sha: "bb1eec7b1376a2437d7069588634d9c40bf985a233b66106b1e5b49c0a473c58",
            bytes: 50322,
            blob: "426a136fc9444fe093a7b3be6ca9ec0037682dcc",
            edits: &[
                (
                    24388,
                    "        let current = if source_path == \"crates/ssx-infra/src/search.rs\" {\n            use causal_successor::a06_source_successor::w07_evidence_successor::{\n                self, SearchOperand,\n            };\n            w07_evidence_successor::search_operand(\n                root,\n                SearchOperand::RuntimePin,\n                &amendment[\"successor\"],\n                &current,\n            )?\n        } else {\n            current\n        };\n",
                    "",
                ),
                (
                    41303,
                    "            } else if path == \"crates/ssx-infra/src/search.rs\" {\n                use causal_successor::a06_source_successor::w07_evidence_successor::{\n                    self, SearchOperand,\n                };\n                w07_evidence_successor::search_operand(\n                    root,\n                    SearchOperand::AdditiveControl,\n                    &entry[\"successor\"],\n                    &actual_current,\n                )?\n",
                    "",
                ),
            ],
        },
    },
];
fn need(ok: bool, why: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(format!("W07/T02 joint source: {why}"))
    }
}
fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn row(path: &str) -> Result<&'static Row> {
    ROWS.iter()
        .find(|r| r.path == path)
        .ok_or_else(|| "W07/T02 unlisted source".into())
}
fn inverse(row: &Row, actual: &[u8], mode: u32, prior: &Prior) -> Result<Vec<u8>> {
    inverse_with_edits(row, actual, mode, prior, prior.edits)
}
fn inverse_with_edits(
    row: &Row,
    actual: &[u8],
    mode: u32,
    prior: &Prior,
    edits: &[(usize, &str, &str)],
) -> Result<Vec<u8>> {
    need(
        mode == 0o644 && actual.len() == row.bytes && sha(actual) == row.sha,
        "exact joint bytes/mode",
    )?;
    let mut out = Vec::new();
    let mut end = 0;
    let mut previous = None;
    for &(at, remove, insert) in edits {
        let next = at.checked_add(remove.len()).ok_or("joint span overflow")?;
        need(
            at >= end
                && previous.is_none_or(|p| at > p)
                && actual.get(at..next) == Some(remove.as_bytes()),
            "ordered joint span",
        )?;
        out.extend_from_slice(&actual[end..at]);
        out.extend_from_slice(insert.as_bytes());
        end = next;
        previous = Some(at);
    }
    out.extend_from_slice(&actual[end..]);
    need(
        out.len() == prior.bytes && sha(&out) == prior.sha,
        "exact parent inverse",
    )?;
    Ok(out)
}
fn read(root: &Path, row: &Row) -> Result<(Vec<u8>, u32)> {
    let full = root.join(row.path);
    let meta = fs::symlink_metadata(&full).map_err(|e| e.to_string())?;
    need(
        meta.is_file() && !meta.file_type().is_symlink(),
        "regular non-symlink source",
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        Ok((
            fs::read(full).map_err(|e| e.to_string())?,
            meta.permissions().mode() & 0o7777,
        ))
    }
    #[cfg(not(unix))]
    {
        Err("W07/T02 source mode proof requires Unix".into())
    }
}
/// The direct dev edge belongs to one exact physical MCP manifest on every lock route.
fn paired_manifest(root: &Path) -> Result<()> {
    let path = root.join("crates/ssx-mcp/Cargo.toml");
    let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    need(
        meta.is_file() && !meta.file_type().is_symlink(),
        "regular non-symlink paired MCP manifest",
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let bytes = fs::read(path).map_err(|e| e.to_string())?;
        need(
            meta.permissions().mode() & 0o7777 == 0o644
                && bytes.len() == 1234
                && sha(&bytes)
                    == "9a077866f6c72e8513d727c4f81928777ee985ae5742ace152bc61875775727b",
            "exact paired MCP manifest bytes/mode",
        )
    }
    #[cfg(not(unix))]
    {
        Err("W07/T02 paired manifest source mode proof requires Unix".into())
    }
}
/// Only the five exact physical afterimages can yield either sealed parent.
/// A parent supplied in place of actual current bytes is always rejected.
pub(crate) fn parent(root: &Path, path: &str, supplied: &[u8], parent: Parent) -> Result<Vec<u8>> {
    let r = row(path)?;
    let (actual, mode) = read(root, r)?;
    need(
        supplied == actual,
        "supplied bytes are not physical joint source",
    )?;
    if path == "Cargo.lock" {
        paired_manifest(root)?;
    }
    let joint = if path == "crates/xtask/src/main.rs" {
        snapshot_fixture_main::joint(root, &actual)?
    } else {
        actual.clone()
    };
    let w07 = inverse(r, &joint, mode, &r.w07)?;
    let t02 = inverse(r, &joint, mode, &r.t02)?;
    Ok(match parent {
        Parent::W07 => w07,
        Parent::T02 => t02,
    })
}
/// Ordinary controls for the shared owner's test wrapper, without discovered tests.
#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn snapshot_fixture_main_controls(root: &Path) -> Result<()> {
    snapshot_fixture_main::controls(root)
}
/// The historical Git identity belongs to the proved parent frame, never to
/// the physical joint file. Both Git bytes and mode/blob must independently agree.
pub(crate) fn parent_git_blob(
    root: &Path,
    path: &str,
    supplied: &[u8],
    selected: Parent,
) -> Result<String> {
    let bytes = parent(root, path, supplied, selected)?;
    let r = row(path)?;
    let p = match selected {
        Parent::W07 => &r.w07,
        Parent::T02 => &r.t02,
    };
    let git = Command::new("git")
        .args(["show", &format!("{}:{}", p.commit, path)])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    need(
        git.status.success() && git.stdout == bytes,
        "proved parent Git bytes",
    )?;
    let tree = Command::new("git")
        .args(["ls-tree", p.commit, "--", path])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    need(
        tree.status.success()
            && tree.stdout == format!("100644 blob {}\t{}\n", p.blob, path).as_bytes(),
        "proved parent Git mode/blob",
    )?;
    Ok(p.blob.into())
}
/// Real Git parents and both exact inverses, plus rejection controls. No new test identity.
pub(crate) fn controls(root: &Path) -> Result<()> {
    need(
        row("crates/xtask/src/main.rs")?.sha == MAIN_SHA && row("Cargo.lock")?.sha == LOCK_SHA,
        "current admission constants",
    )?;
    for r in ROWS {
        let (actual, mode) = read(root, r)?;
        let joint = if r.path == "crates/xtask/src/main.rs" {
            snapshot_fixture_main::joint(root, &actual)?
        } else {
            actual.clone()
        };
        for (kind, p) in [(Parent::W07, &r.w07), (Parent::T02, &r.t02)] {
            let prior = parent(root, r.path, &actual, kind)?;
            need(
                parent_git_blob(root, r.path, &actual, kind)? == p.blob,
                "same proved parent Git frame",
            )?;
            let git = Command::new("git")
                .args(["show", &format!("{}:{}", p.commit, r.path)])
                .current_dir(root)
                .output()
                .map_err(|e| e.to_string())?;
            need(
                git.status.success() && git.stdout == prior,
                "parent Git bytes",
            )?;
            let tree = Command::new("git")
                .args(["ls-tree", p.commit, "--", r.path])
                .current_dir(root)
                .output()
                .map_err(|e| e.to_string())?;
            need(
                tree.status.success()
                    && tree.stdout == format!("100644 blob {}\t{}\n", p.blob, r.path).as_bytes(),
                "parent Git mode/blob",
            )?;
            need(
                parent(root, r.path, &prior, kind).is_err(),
                "historical parent supplied as physical current",
            )?;
            let mut changed = actual.clone();
            changed.push(0);
            need(
                parent(root, r.path, &changed, kind).is_err(),
                "changed supplied current",
            )?;
            for bad_mode in [0o600, 0o755, 0o4644] {
                need(
                    inverse(r, &joint, bad_mode, p).is_err(),
                    "changed source mode",
                )?;
            }
            for (index, &(at, remove, insert)) in p.edits.iter().enumerate() {
                if remove.is_empty() {
                    need(!insert.is_empty(), "vacuous joint span")?;
                    let mut omitted = p.edits.to_vec();
                    omitted.remove(index);
                    let mut duplicate = p.edits.to_vec();
                    duplicate.insert(index, p.edits[index]);
                    let mut moved = p.edits.to_vec();
                    moved[index].0 = at.checked_add(1).ok_or("joint control offset overflow")?;
                    for edits in [omitted, duplicate, moved] {
                        need(
                            edits.as_slice() != p.edits,
                            "unchanged joint edit descriptor",
                        )?;
                        need(
                            inverse_with_edits(r, &joint, mode, p, &edits).is_err(),
                            "missing/duplicate/relocated insertion-only joint edit",
                        )?;
                    }
                    continue;
                }
                let mut missing = joint.clone();
                missing.drain(at..at + remove.len());
                let mut duplicate = joint.clone();
                duplicate.splice(at..at, remove.as_bytes().iter().copied());
                let mut moved = missing.clone();
                moved.extend_from_slice(remove.as_bytes());
                for bad in [missing, duplicate, moved] {
                    need(
                        bad != joint && inverse(r, &bad, mode, p).is_err(),
                        "missing/duplicate/relocated joint span",
                    )?;
                }
                need(
                    !remove.is_empty() || !insert.is_empty(),
                    "vacuous joint span",
                )?;
            }
        }
    }
    need(
        parent(root, "unknown", b"", Parent::W07).is_err(),
        "unknown source path",
    )?;
    filesystem_controls(root)
}

#[cfg(not(unix))]
fn filesystem_controls(_: &Path) -> Result<()> {
    Err("W07/T02 filesystem source proof requires Unix".into())
}
#[cfg(unix)]
fn filesystem_controls(root: &Path) -> Result<()> {
    use std::os::unix::fs::{PermissionsExt, symlink};
    struct Owned(std::path::PathBuf);
    impl Drop for Owned {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let directory = std::env::temp_dir().join(format!("w07-joint-{}-{suffix}", std::process::id()));
    fs::create_dir(&directory).map_err(|e| e.to_string())?;
    let owned = Owned(directory);
    let manifest_path = owned.0.join("crates/ssx-mcp/Cargo.toml");
    fs::create_dir_all(manifest_path.parent().ok_or("paired manifest parent")?)
        .map_err(|e| e.to_string())?;
    let manifest = fs::read(root.join("crates/ssx-mcp/Cargo.toml")).map_err(|e| e.to_string())?;
    fs::write(&manifest_path, &manifest).map_err(|e| e.to_string())?;
    fs::set_permissions(&manifest_path, fs::Permissions::from_mode(0o644))
        .map_err(|e| e.to_string())?;
    let capsule_relative = "docs/rust-rewrite/snapshot-fixture-amendment/capsule.json";
    let capsule_path = owned.0.join(capsule_relative);
    fs::create_dir_all(capsule_path.parent().ok_or("capsule parent")?)
        .map_err(|e| e.to_string())?;
    fs::copy(root.join(capsule_relative), &capsule_path).map_err(|e| e.to_string())?;
    fs::set_permissions(&capsule_path, fs::Permissions::from_mode(0o644))
        .map_err(|e| e.to_string())?;
    for r in ROWS {
        let (actual, _) = read(root, r)?;
        let path = owned.0.join(r.path);
        fs::create_dir_all(path.parent().ok_or("joint owned source parent")?)
            .map_err(|e| e.to_string())?;
        fs::write(&path, &actual).map_err(|e| e.to_string())?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).map_err(|e| e.to_string())?;
        parent(&owned.0, r.path, &actual, Parent::W07)?;
        if r.path == "crates/xtask/src/main.rs" {
            let old_joint = snapshot_fixture_main::joint(root, &actual)?;
            fs::write(&path, &old_joint).map_err(|e| e.to_string())?;
            need(
                parent(&owned.0, r.path, &old_joint, Parent::W07).is_err(),
                "old joint physically substituted for new current",
            )?;
            fs::write(&path, &actual).map_err(|e| e.to_string())?;
        }
        for bad_mode in [0o600, 0o4644] {
            fs::set_permissions(&path, fs::Permissions::from_mode(bad_mode))
                .map_err(|e| e.to_string())?;
            need(
                parent(&owned.0, r.path, &actual, Parent::W07).is_err(),
                "physical source mode accepted",
            )?;
        }
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).map_err(|e| e.to_string())?;
        let backup = path.with_extension("owned-backup");
        fs::rename(&path, &backup).map_err(|e| e.to_string())?;
        need(
            parent(&owned.0, r.path, &actual, Parent::W07).is_err(),
            "missing physical source accepted",
        )?;
        symlink(&backup, &path).map_err(|e| e.to_string())?;
        need(
            parent(&owned.0, r.path, &actual, Parent::W07).is_err(),
            "symlink physical source accepted",
        )?;
        fs::remove_file(&path).map_err(|e| e.to_string())?;
        fs::create_dir(&path).map_err(|e| e.to_string())?;
        need(
            parent(&owned.0, r.path, &actual, Parent::W07).is_err(),
            "nonregular physical source accepted",
        )?;
        fs::remove_dir(&path).map_err(|e| e.to_string())?;
        fs::rename(backup, path).map_err(|e| e.to_string())?;
    }
    let lock = fs::read(owned.0.join("Cargo.lock")).map_err(|e| e.to_string())?;
    for kind in [Parent::W07, Parent::T02] {
        let mut changed = manifest.clone();
        changed[0] ^= 1;
        fs::write(&manifest_path, changed).map_err(|e| e.to_string())?;
        need(
            parent(&owned.0, "Cargo.lock", &lock, kind).is_err(),
            "wrong paired manifest accepted",
        )?;
        fs::write(&manifest_path, &manifest).map_err(|e| e.to_string())?;
        for mode in [0o600, 0o755, 0o4644] {
            fs::set_permissions(&manifest_path, fs::Permissions::from_mode(mode))
                .map_err(|e| e.to_string())?;
            need(
                parent(&owned.0, "Cargo.lock", &lock, kind).is_err(),
                "paired manifest mode accepted",
            )?;
        }
        fs::set_permissions(&manifest_path, fs::Permissions::from_mode(0o644))
            .map_err(|e| e.to_string())?;
        let backup = manifest_path.with_extension("owned-backup");
        fs::rename(&manifest_path, &backup).map_err(|e| e.to_string())?;
        need(
            parent(&owned.0, "Cargo.lock", &lock, kind).is_err(),
            "missing paired manifest accepted",
        )?;
        symlink(&backup, &manifest_path).map_err(|e| e.to_string())?;
        need(
            parent(&owned.0, "Cargo.lock", &lock, kind).is_err(),
            "symlink paired manifest accepted",
        )?;
        fs::remove_file(&manifest_path).map_err(|e| e.to_string())?;
        fs::create_dir(&manifest_path).map_err(|e| e.to_string())?;
        need(
            parent(&owned.0, "Cargo.lock", &lock, kind).is_err(),
            "nonregular paired manifest accepted",
        )?;
        fs::remove_dir(&manifest_path).map_err(|e| e.to_string())?;
        fs::rename(backup, &manifest_path).map_err(|e| e.to_string())?;
        parent(&owned.0, "Cargo.lock", &lock, kind)?;
    }
    Ok(())
}
