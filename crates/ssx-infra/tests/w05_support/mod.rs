//! Native W00 contract tests use only isolated private Valkey, never engines.
use ssx_application::{
    policy::{PolicyService, SearchPolicy},
    saved::*,
    scope::{CatalogSnapshot, EngineCapability, ScopeResolver},
};
use ssx_domain::{LeaseToken, SavedSearchId};
use ssx_infra::valkey::{saved::*, *};
use ssx_json::json;
use std::{
    collections::BTreeMap,
    fs,
    io::Read as _,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
pub struct Server {
    child: Child,
    dir: PathBuf,
    pub url: String,
}
impl Server {
    pub fn start() -> Self {
        let mut bytes = [0; 16];
        fs::File::open("/dev/urandom")
            .unwrap()
            .read_exact(&mut bytes)
            .unwrap();
        bytes[6] = (bytes[6] & 15) | 64;
        bytes[8] = (bytes[8] & 63) | 128;
        let id: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let dir = std::env::temp_dir().join(format!("ssx-w05-{id}"));
        fs::create_dir(&dir).unwrap();
        let url = format!("unix://{}", dir.join("s.sock").display());
        let mut s = Self {
            child: Self::spawn(&dir),
            dir,
            url,
        };
        s.wait_ready();
        s
    }
    fn spawn(dir: &std::path::Path) -> Child {
        Command::new(
            std::env::var("SLOPSEARX_TEST_VALKEY_SERVER")
                .unwrap_or_else(|_| "valkey-server".into()),
        )
        .args([
            "--port",
            "0",
            "--bind",
            "127.0.0.1",
            "--save",
            "",
            "--appendonly",
            "no",
            "--daemonize",
            "no",
            "--unixsocketperm",
            "700",
            "--dir",
        ])
        .arg(dir)
        .arg("--unixsocket")
        .arg(dir.join("s.sock"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("disposable valkey-server required")
    }
    fn wait_ready(&mut self) {
        let until = Instant::now() + Duration::from_secs(4);
        loop {
            assert!(
                self.child.try_wait().unwrap().is_none(),
                "Valkey startup failed"
            );
            if redis::Client::open(self.url.as_str())
                .unwrap()
                .get_connection()
                .is_ok()
            {
                break;
            }
            assert!(Instant::now() < until, "Valkey startup deadline");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    pub fn connection(&self) -> redis::Connection {
        redis::Client::open(self.url.as_str())
            .unwrap()
            .get_connection()
            .unwrap()
    }
    pub fn store(&self) -> Valkey {
        Valkey::new(&self.url, Duration::from_secs(2)).unwrap()
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.dir);
    }
}
pub struct Tokens(pub &'static str);
impl TokenSource<SavedSearchId> for Tokens {
    fn next_token(&mut self) -> LeaseToken<SavedSearchId> {
        LeaseToken::new(self.0).unwrap()
    }
}
pub fn id(name: &str) -> SavedSearchId {
    SavedSearchId::new(name).unwrap()
}
pub fn policy() -> PolicyService {
    PolicyService {
        policy: SearchPolicy::default(),
        resolver: ScopeResolver::default(),
        catalog: CatalogSnapshot::new(vec![EngineCapability {
            name: "brave".into(),
            enabled: true,
            active: true,
            available: false,
            categories: vec!["general".into()],
            media_types: vec![],
            auth_ready: true,
            circuit_open: false,
            cost_class: "free".into(),
            health_status: "ok".into(),
            health_stale: false,
            enforced_filters: BTreeMap::new(),
        }])
        .unwrap(),
    }
}
pub fn create_at(p: &SavedPolicy<'_>, tenant: &str, name: &str, now: f64) -> PreparedDefinition {
    prepare_create(p,tenant,name,&json!({"query":"q","engines":["brave"],"interval_seconds":60,"start_immediately":true,"expires_in_seconds":600}),now).unwrap()
}
pub fn publication(claim: &Claim, now: f64) -> (ssx_json::Value, Option<ssx_json::Value>, f64) {
    let d = claim.definition();
    let slot = d.slot(now).unwrap();
    let observation = observe(
        d,
        &ssx_domain::SearchResponse {
            scope: ssx_domain::ScopeDecision {
                selected_engines: d.engines.clone(),
                ..Default::default()
            },
            engine_outcomes: vec![ssx_domain::EngineOutcome {
                engine: "brave".into(),
                status: "ok".into(),
                ..Default::default()
            }],
            ..Default::default()
        },
        now,
    )
    .unwrap();
    let (mut report, baseline) = compare_observations(
        d,
        observation,
        &slot.run_id,
        slot.scheduled_at.get(),
        now,
        now,
        slot.missed_slots,
    )
    .unwrap();
    report["slot"] = json!(slot.slot);
    (report, baseline, slot.following_due.get())
}
