//! Standalone external-HTTP client. No service/domain/backend wiring.
mod cli_float;
mod compat_json;
mod decimal;
#[cfg(feature = "source-transport-test-fixtures")]
mod diagnostics;
mod proxy;
mod source_io;
mod tls;
mod transport;
use clap::{CommandFactory, Parser, Subcommand};
use compat_json::{Value, string, text};
use std::{collections::BTreeMap, process::ExitCode};
#[derive(Debug)]
enum Failure {
    Transport,
    Http(u16),
    Utf8,
    Json,
    Shape,
    Encoding,
    Integer,
    Trust,
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Transport => "request failed",
            Self::Http(_) => "HTTP failure",
            Self::Utf8 => "response is not UTF-8",
            Self::Json => "invalid JSON response",
            Self::Shape => "unexpected JSON shape",
            Self::Encoding => "output contains an unpaired Unicode surrogate",
            Self::Integer => "integer exceeds 4300 decimal digits",
            Self::Trust => "invalid CA configuration",
        })?;
        if let Self::Http(status) = self {
            write!(f, ": {status}")?;
        }
        Ok(())
    }
}
#[derive(Parser)]
#[command(
    name = "ssx",
    bin_name = "ssx",
    about = "SlopSearX CLI — AI-agent-native meta search client",
    after_help = "Environment: SSX_URL, SSX_FORMAT (yaml|json). See docs/rust-rewrite/t08a/README.md.",
    infer_long_args = true,
    args_override_self = true
)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}
#[derive(Subcommand)]
enum Commands {
    /// Execute a search query
    #[command(infer_long_args = true, args_override_self = true)]
    Search {
        #[arg(required=true,num_args=1..,allow_negative_numbers=true)]
        query: Vec<String>,
        #[arg(long, short = 'c', allow_hyphen_values = true)]
        categories: Option<String>,
        #[arg(long, short = 'e', allow_hyphen_values = true)]
        engines: Option<String>,
        #[arg(long, short = 'l', default_value = "en", allow_hyphen_values = true)]
        language: String,
        #[arg(
            long,
            help = "Time range: day, month, year",
            allow_hyphen_values = true
        )]
        time_range: Option<String>,
        #[arg(long,default_value="1",allow_hyphen_values=true,value_parser=integer)]
        pageno: String,
        #[arg(long)]
        json: bool,
    },
    /// List engines with status
    #[command(infer_long_args = true, args_override_self = true)]
    Engines {
        #[arg(long)]
        json: bool,
    },
    /// Check server and engine health
    #[command(infer_long_args = true, args_override_self = true)]
    Health {
        #[arg(long)]
        json: bool,
    },
    /// Show engine-to-categories mapping
    #[command(infer_long_args = true, args_override_self = true)]
    Config {
        #[arg(long)]
        json: bool,
    },
}
fn python_space(c: char) -> bool {
    c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}')
}
fn integer(raw: &str) -> Result<String, String> {
    let mut chars = raw.trim_matches(char::is_whitespace).chars().peekable();
    let negative = match chars.peek() {
        Some('-') => {
            chars.next();
            true
        }
        Some('+') => {
            chars.next();
            false
        }
        _ => false,
    };
    let mut digits = String::new();
    let mut previous_digit = false;
    for c in chars {
        if c == '_' && previous_digit {
            previous_digit = false;
            continue;
        }
        let cp = u32::from(c);
        let digit = decimal::ZEROES
            .iter()
            .find_map(|&z| (z..z + 10).contains(&cp).then(|| (cp - z) as u8));
        let Some(digit) = digit else {
            return Err("invalid integer syntax".into());
        };
        digits.push(char::from(b'0' + digit));
        previous_digit = true;
    }
    if !previous_digit {
        return Err("invalid integer syntax".into());
    }
    if digits.len() > 4300 {
        return Err("integer exceeds 4300 decimal digits".into());
    }
    let digits = digits.trim_start_matches('0');
    Ok(if digits.is_empty() {
        "0".into()
    } else {
        format!("{}{digits}", if negative { "-" } else { "" })
    })
}
fn encoded(value: &str) -> String {
    let mut out = String::new();
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-' | b'~' => {
                out.push(char::from(b))
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
fn url(path: &str, params: &[(&str, String)]) -> Result<String, Failure> {
    let base = match std::env::var("SSX_URL") {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => "http://localhost:8080".into(),
        Err(std::env::VarError::NotUnicode(_)) => return Err(Failure::Transport),
    };
    let mut out = format!("{}{path}", base.trim_end_matches('/'));
    for (i, (k, v)) in params.iter().enumerate() {
        out.push(if i == 0 { '?' } else { '&' });
        out.push_str(k);
        out.push('=');
        out.push_str(&encoded(v));
    }
    Ok(out)
}
fn use_json(flag: bool) -> bool {
    flag || std::env::var("SSX_FORMAT").is_ok_and(|s| s.to_lowercase() == "json")
}
async fn fetch_json(path: &str) -> Result<Value, Failure> {
    compat_json::parse(&transport::fetch(&url(path, &[])?, "application/json").await?)
}
fn default<'a>(value: &'a Value, key: &str, fallback: &'a Value) -> Result<&'a Value, Failure> {
    Ok(value.get(key)?.unwrap_or(fallback))
}
fn sorted_object(value: &Value) -> Result<Vec<(&Vec<u32>, &Value)>, Failure> {
    let mut v = value
        .object()?
        .iter()
        .map(|(k, v)| (k, v))
        .collect::<Vec<_>>();
    v.sort_by(|a, b| a.0.cmp(b.0));
    Ok(v)
}
fn health(value: &Value) -> Result<String, Failure> {
    let unknown = string("unknown");
    let empty = Value::Object(vec![]);
    let engines = default(value, "engines", &empty)?;
    let entries = sorted_object(engines)?;
    let ok = entries.iter().try_fold(0, |count, (_, v)| {
        Ok::<_, Failure>(count + usize::from(v.get("status")? == Some(&string("ok"))))
    })?;
    let mut out = format!(
        "---\nserver_status: {}\nengines_ok: {ok}/{}\nengine_status:\n",
        default(value, "status", &unknown)?.display()?,
        entries.len()
    );
    for (k, v) in entries {
        out.push_str(&format!(
            "  {}: {}\n",
            compat_json::points(k)?,
            default(v, "status", &unknown)?.display()?
        ));
    }
    Ok(out)
}
fn config(value: &Value) -> Result<String, Failure> {
    let empty = Value::Object(vec![]);
    let categories = default(value, "categories", &empty)?;
    let entries = sorted_object(categories)?;
    let mut out = format!("---\ncategory_count: {}\ncategories:\n", entries.len());
    for (k, v) in entries {
        out.push_str(&format!("  {}:\n", compat_json::points(k)?));
        let engines = v.sorted_sequence()?;
        for e in engines {
            out.push_str(&format!("    - {}\n", e.display()?));
        }
    }
    out.push_str("\n## Engine Category Reference\n\nEach engine can be queried with `?engines=<name>` or reached\nvia any of its assigned categories with `?categories=<cat>`.\n");
    Ok(out)
}
fn engines(health: &Value, config: &Value, json: bool) -> Result<String, Failure> {
    let empty = Value::Object(vec![]);
    let unknown = string("unknown");
    let states = default(health, "engines", &empty)?;
    let categories = default(config, "categories", &empty)?;
    let mut mapping: BTreeMap<Vec<u32>, Vec<Vec<u32>>> = BTreeMap::new();
    for (k, v) in categories.object()? {
        for engine in v.sequence()? {
            match &engine {
                Value::String(e) => mapping.entry(e.clone()).or_default().push(k.clone()),
                Value::Null | Value::Bool(_) | Value::Number(_) => {}
                _ => return Err(Failure::Shape),
            }
        }
    }
    for cats in mapping.values_mut() {
        cats.sort();
    }
    let entries = sorted_object(states)?;
    let mut output = Vec::new();
    let mut yaml = if json {
        String::new()
    } else {
        format!(
            "---\nserver_status: {}\nengine_count: {}\nengines:\n",
            default(health, "status", &unknown)?.display()?,
            entries.len()
        )
    };
    for (k, v) in entries {
        let status = default(v, "status", &unknown)?;
        let cats = mapping.get(k).cloned().unwrap_or_default();
        if json {
            output.push((
                k.clone(),
                Value::Object(vec![
                    (text("status"), status.clone()),
                    (
                        text("categories"),
                        Value::Array(cats.into_iter().map(Value::String).collect()),
                    ),
                ]),
            ));
        } else {
            yaml.push_str(&format!(
                "  - name: {}\n    status: {}\n    categories: [{}]\n",
                compat_json::points(k)?,
                status.display()?,
                cats.iter()
                    .map(|v| compat_json::points(v))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(", ")
            ));
        }
    }
    Ok(if json {
        compat_json::pretty(&Value::Object(output))
    } else {
        yaml
    })
}
async fn run(args: Args) -> Result<String, Failure> {
    match args.command {
        Commands::Search {
            query,
            categories,
            engines,
            language,
            time_range,
            pageno,
            json,
        } => {
            let query = query.join(" ");
            if query.trim_matches(python_space).is_empty() {
                return Ok(String::new());
            }
            let mut params = vec![("q", query)];
            for (k, v) in [
                ("categories", categories),
                ("engines", engines),
                ("language", Some(language)),
                ("time_range", time_range),
                ("pageno", (pageno != "0").then_some(pageno)),
            ] {
                if let Some(v) = v.filter(|s| !s.is_empty()) {
                    params.push((k, v));
                }
            }
            let json = use_json(json);
            params.push(("format", if json { "json" } else { "yaml" }.into()));
            let raw = transport::fetch(
                &url("/search", &params)?,
                if json {
                    "application/json"
                } else {
                    "text/vnd.yaml+markdown"
                },
            )
            .await?;
            if json {
                Ok(compat_json::pretty(&compat_json::parse(&raw)?))
            } else {
                Ok(raw + "\n")
            }
        }
        Commands::Health { json } => {
            let data = fetch_json("/health").await?;
            if use_json(json) {
                Ok(compat_json::pretty(&data))
            } else {
                health(&data)
            }
        }
        Commands::Config { json } => {
            let data = fetch_json("/config").await?;
            if use_json(json) {
                Ok(compat_json::pretty(&data))
            } else {
                config(&data)
            }
        }
        Commands::Engines { json } => {
            let h = fetch_json("/health").await?;
            let c = fetch_json("/config").await?;
            engines(&h, &c, use_json(json))
        }
    }
}
// Clap owns typing/help. This source-grammar adapter preserves argparse's
// contiguous nargs='+' block, negative-token admission, last-option wins, and
// Unicode decimal query tokens without letting Clap's broader exponent grammar
// introduce a request that the source rejects. Only argument order changes here;
// exact original query token bytes/order and option values are retained.
fn negative_token(arg: &str) -> bool {
    let Some(value) = arg.strip_prefix('-') else {
        return false;
    };
    let digits = |s: &str| {
        s.chars().all(|c| {
            crate::decimal::ZEROES
                .iter()
                .any(|&z| (z..z + 10).contains(&u32::from(c)))
        })
    };
    if !value.is_empty() && digits(value) {
        return true;
    }
    value
        .split_once('.')
        .is_some_and(|(a, b)| !b.is_empty() && digits(a) && digits(b))
}
fn source_argv() -> Result<Vec<std::ffi::OsString>, &'static str> {
    let original = std::env::args_os().collect::<Vec<_>>();
    let raw = original
        .iter()
        .cloned()
        .map(|v| v.into_string())
        .collect::<Result<Vec<_>, _>>();
    let Ok(raw) = raw else { return Ok(original) };
    if raw.get(1).map(String::as_str) != Some("search") {
        return Ok(original);
    }
    let names = [
        "categories",
        "engines",
        "language",
        "time-range",
        "pageno",
        "json",
        "help",
    ];
    let mut options = Vec::new();
    let mut query = Vec::new();
    let mut started = false;
    let mut closed = false;
    let mut end = false;
    let mut at = 2;
    while let Some(arg) = raw.get(at) {
        if !end && arg == "--" {
            end = true;
            at += 1;
            continue;
        }
        if !end && arg.starts_with('-') && arg != "-" && !negative_token(arg) {
            let mut takes_value = false;
            let mut attached = false;
            if let Some(long) = arg.strip_prefix("--") {
                let name = long.split('=').next().unwrap_or(long);
                let matches = names
                    .iter()
                    .filter(|n| n.starts_with(name))
                    .collect::<Vec<_>>();
                if matches.len() != 1 {
                    return Err("unrecognized or ambiguous argument");
                }
                if *matches[0] == "help" {
                    return Ok(original);
                }
                takes_value = *matches[0] != "json";
                attached = long.contains('=');
            } else if let Some(short) = arg.chars().nth(1) {
                match short {
                    'c' | 'e' | 'l' => {
                        takes_value = true;
                        attached = arg.len() > 2;
                    }
                    'h' => return Ok(original),
                    _ => return Err("unrecognized argument"),
                }
            }
            closed |= started;
            options.push(arg.clone());
            if takes_value && !attached {
                at += 1;
                let value = raw.get(at).ok_or("expected an option value")?;
                if value.starts_with('-') && value != "-" && !negative_token(value) {
                    return Err("expected an option value");
                };
                options.push(value.clone());
            }
        } else {
            if closed {
                return Err("query tokens must form one contiguous argument block");
            };
            started = true;
            query.push(arg.clone());
        }
        at += 1;
    }
    let mut normalized = vec![raw[0].clone(), "search".into()];
    normalized.extend(options);
    normalized.push("--".into());
    normalized.extend(query);
    Ok(normalized
        .into_iter()
        .map(std::ffi::OsString::from)
        .collect())
}
#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let argv = source_argv().unwrap_or_else(|message| {
        Args::command()
            .error(clap::error::ErrorKind::UnknownArgument, message)
            .exit()
    });
    let args = Args::parse_from(argv);
    if matches!(&args.command,Commands::Search{query,..} if query.join(" ").trim_matches(python_space).is_empty())
    {
        eprintln!("error: query is required");
        return ExitCode::FAILURE;
    }
    match run(args).await {
        Ok(output) => {
            use std::io::Write;
            match std::io::stdout().lock().write_all(output.as_bytes()) {
                Ok(()) => ExitCode::SUCCESS,
                Err(_) => {
                    eprintln!("error: ssx: output failed");
                    ExitCode::FAILURE
                }
            }
        }
        Err(e) => {
            eprintln!("error: ssx: {e}");
            ExitCode::FAILURE
        }
    }
}
