//! Exact Windows three-executable native package; no PATH/helper overrides.
#![forbid(unsafe_code)]
use super::SourceResolverHelper;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ssx_windows_owner::{
    ImageRole, TrustedImage, build_file_digest, check_private_directory, create_private_directory,
    create_private_file, package_file_digest, package_lock, read_package_file,
};
use std::{
    fs::OpenOptions,
    io::{self, Write},
    path::{Path, PathBuf},
};
const RECORD: &str = "share/slopsearx/native-install.json";
const GATEWAY: &str = "bin/ssx-gateway.exe";
const DNS: &str = "libexec/slopsearx/ssx-gateway-dns.exe";
const INPUT: &str = "libexec/slopsearx/ssx-gateway-input.exe";
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InstallRecord {
    pub schema: u32,
    pub resolver_protocol: u32,
    pub package: String,
    pub version: String,
    pub os: String,
    pub arch: String,
    pub gateway_path: String,
    pub gateway_bytes: u64,
    pub gateway_sha256: String,
    pub helper_path: String,
    pub helper_bytes: u64,
    pub helper_sha256: String,
    pub input_helper_path: String,
    pub input_helper_bytes: u64,
    pub input_helper_sha256: String,
}
fn invalid() -> io::Error {
    io::ErrorKind::InvalidData.into()
}
fn need(ok: bool) -> io::Result<()> {
    if ok { Ok(()) } else { Err(invalid()) }
}
fn hex(raw: &[u8]) -> String {
    raw.iter().map(|b| format!("{b:02x}")).collect()
}
fn decode(raw: &str) -> io::Result<[u8; 32]> {
    need(raw.len() == 64 && raw.bytes().all(|b| b.is_ascii_hexdigit()))?;
    let mut bytes = [0; 32];
    for (i, p) in bytes.iter_mut().enumerate() {
        *p = u8::from_str_radix(&raw[i * 2..i * 2 + 2], 16).map_err(|_| invalid())?;
    }
    Ok(bytes)
}
fn prefix(exe: &Path) -> io::Result<PathBuf> {
    need(exe.is_absolute() && exe.file_name().is_some_and(|s| s == "ssx-gateway.exe"))?;
    let bin = exe.parent().ok_or_else(invalid)?;
    need(bin.file_name().is_some_and(|s| s == "bin"))?;
    Ok(bin.parent().ok_or_else(invalid)?.into())
}
fn images(executable: &Path) -> io::Result<(SourceResolverHelper, TrustedImage)> {
    let root = prefix(executable)?;
    for n in [
        "",
        "bin",
        "libexec",
        "libexec/slopsearx",
        "share",
        "share/slopsearx",
    ] {
        check_private_directory(&root.join(n))?;
    }
    let raw = read_package_file(&root.join(RECORD), 16384)?;
    let record: InstallRecord = serde_json::from_slice(&raw).map_err(|_| invalid())?;
    need(
        record.schema == 2
            && record.resolver_protocol == 2
            && record.package == "slopsearx-native-gateway"
            && record.version == env!("CARGO_PKG_VERSION")
            && record.os == "windows"
            && record.arch == "x86_64"
            && std::env::consts::ARCH == "x86_64"
            && record.gateway_path == GATEWAY
            && record.helper_path == DNS
            && record.input_helper_path == INPUT,
    )?;
    let actual = package_file_digest(executable)?;
    need(actual.bytes == record.gateway_bytes && actual.sha256 == decode(&record.gateway_sha256)?)?;
    let helper = root.join(DNS);
    let digest = package_file_digest(&helper)?;
    need(digest.bytes == record.helper_bytes && digest.sha256 == decode(&record.helper_sha256)?)?;
    let input = root.join(INPUT);
    let digest = package_file_digest(&input)?;
    need(
        digest.bytes == record.input_helper_bytes
            && digest.sha256 == decode(&record.input_helper_sha256)?,
    )?;
    Ok((
        SourceResolverHelper::new(helper, decode(&record.helper_sha256)?)?,
        TrustedImage::open(
            input,
            decode(&record.input_helper_sha256)?,
            ImageRole::Input,
        )?,
    ))
}
pub fn discover(executable: &Path) -> io::Result<SourceResolverHelper> {
    images(executable).map(|p| p.0)
}
pub fn current_helper() -> io::Result<SourceResolverHelper> {
    discover(&std::env::current_exe()?)
}
pub(crate) fn current_input_image() -> io::Result<TrustedImage> {
    images(&std::env::current_exe()?).map(|p| p.1)
}
/// Windows requires all three explicit Cargo-emitted paths; no guessed sibling.
pub fn stage(gateway: &Path, helper: &Path, input: &Path, output: &Path) -> io::Result<PathBuf> {
    need(
        [gateway, helper, input, output]
            .iter()
            .all(|p| p.is_absolute()),
    )?;
    check_private_directory(output)?;
    let g = build_file_digest(gateway)?;
    let d = build_file_digest(helper)?;
    let i = build_file_digest(input)?;
    let record = InstallRecord {
        schema: 2,
        resolver_protocol: 2,
        package: "slopsearx-native-gateway".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        os: "windows".into(),
        arch: "x86_64".into(),
        gateway_path: GATEWAY.into(),
        gateway_bytes: g.bytes,
        gateway_sha256: hex(&g.sha256),
        helper_path: DNS.into(),
        helper_bytes: d.bytes,
        helper_sha256: hex(&d.sha256),
        input_helper_path: INPUT.into(),
        input_helper_bytes: i.bytes,
        input_helper_sha256: hex(&i.sha256),
    };
    let mut raw = serde_json::to_vec_pretty(&record).map_err(|_| invalid())?;
    raw.push(b'\n');
    need(raw.len() <= 16384)?;
    let _lock = package_lock(&output.join(".gateway-package.lock"))?;
    let root = output.join(format!("ssx-gateway-{}", hex(&Sha256::digest(&raw))));
    if root.exists() {
        need(read_package_file(&root.join(RECORD), 16384)? == raw)?;
        images(&root.join(GATEWAY))?;
        return Ok(root.join(GATEWAY));
    }
    create_private_directory(&root)?;
    for n in [
        "bin",
        "libexec",
        "libexec/slopsearx",
        "share",
        "share/slopsearx",
    ] {
        create_private_directory(&root.join(n))?;
        check_private_directory(&root.join(n))?;
    }
    for (source, name, expected) in [(gateway, GATEWAY, g), (helper, DNS, d), (input, INPUT, i)] {
        let mut source = OpenOptions::new().read(true).open(source)?;
        let mut target = create_private_file(&root.join(name))?;
        let copied = io::copy(
            &mut std::io::Read::take(&mut source, 512 * 1024 * 1024 + 1),
            &mut target,
        )?;
        target.sync_all()?;
        need(copied == expected.bytes)?;
        drop(target);
        need(build_file_digest(&root.join(name))? == expected)?;
    }
    let mut file = create_private_file(&root.join(RECORD))?;
    file.write_all(&raw)?;
    file.sync_all()?;
    drop(file);
    images(&root.join(GATEWAY))?;
    Ok(root.join(GATEWAY))
}
pub fn stage_cargo_gateway(gateway: &Path, helper: &Path, input: &Path) -> io::Result<PathBuf> {
    let root = std::env::var_os("SLOPSEARX_INSTALL_TEST_ROOT")
        .map(PathBuf::from)
        .ok_or_else(invalid)?;
    stage(gateway, helper, input, &root)
}
pub fn stage_distribution(
    gateway: &Path,
    helper: &Path,
    input: &Path,
    output: &Path,
) -> io::Result<PathBuf> {
    stage(gateway, helper, input, output)
}

/// Exact production custody reader for an already existing output directory.
pub fn check_output_root(path: &Path) -> io::Result<()> {
    ssx_windows_owner::check_private_directory(path)
}

pub fn prepare_output_root(path: &Path) -> io::Result<()> {
    create_private_directory(path)
}

#[cfg(feature = "gateway-dns-fixtures")]
#[doc(hidden)]
pub fn input_image_for_fixture(executable: &Path, expected: [u8; 32]) -> io::Result<TrustedImage> {
    let root = prefix(executable)?;
    let (_, input) = images(executable)?;
    need(package_file_digest(&root.join(INPUT))?.sha256 == expected)?;
    Ok(input)
}
