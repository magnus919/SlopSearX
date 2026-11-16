//! Offline, exact-source vendor build. Never discovers a host libxml2.
use sha2::{Digest, Sha256};
use std::{env, fs, path::PathBuf, process::Command};
fn hex_digest(bytes: &[u8]) -> String {
  Sha256::digest(bytes)
    .iter()
    .map(|b| format!("{b:02x}"))
    .collect()
}
fn run(command: &mut Command) {
  let status = command
    .status()
    .expect("required native build tool unavailable");
  assert!(
    status.success(),
    "native vendor command failed: {command:?}"
  );
}
fn main() {
  let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
  let archive = manifest.join("../libxml2/libxml2-2.15.4.tar.xz");
  let bytes = fs::read(&archive).expect("vendored libxml2 source is required; no fallback");
  let digest = Sha256::digest(&bytes)
    .iter()
    .map(|b| format!("{b:02x}"))
    .collect::<String>();
  assert_eq!(
    digest, "98087fd181d9070724f3fbc65c7377db03038eb92bd882374daff44940138821",
    "vendor source drift"
  );
  let host = env::var("HOST").unwrap();
  let target = env::var("TARGET").unwrap();
  assert_eq!(
    host, target,
    "libxml2 must be built natively per qualified target; no host/cross fallback"
  );
  assert!(
    target.ends_with("-apple-darwin") || target.ends_with("-unknown-linux-gnu"),
    "unqualified backend target"
  );
  assert!(
    env::var_os("LIBXML2").is_none(),
    "LIBXML2 override forbidden"
  );
  let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
  let src = out.join("libxml2-2.15.4");
  let build = out.join("native-build");
  let prefix = out.join("native-prefix");
  if !src.is_dir() {
    run(
      Command::new("tar")
        .arg("-xJf")
        .arg(&archive)
        .arg("-C")
        .arg(&out),
    );
  }
  assert_eq!(
    fs::read_to_string(src.join("VERSION")).unwrap().trim(),
    "2.15.4"
  );
  let patch = manifest.join("../libxml2/a05-normal-data-nul.patch");
  let patch_bytes = fs::read(&patch).expect("required compatibility patch");
  assert_eq!(
    hex_digest(&patch_bytes),
    "a0817f232192bc74082dd40c39334312e0102c5a20d956e654e6eab5dba35b68",
    "compatibility patch drift"
  );
  let parser_hash = hex_digest(&fs::read(src.join("HTMLparser.c")).unwrap());
  if parser_hash == "39b16dc8a54828427044b5b29d78c5c7c39ae9d06f057712b7b4e851bec2f6da" {
    run(
      Command::new("patch")
        .arg("--batch")
        .arg("-p1")
        .arg("-i")
        .arg(&patch)
        .current_dir(&src),
    );
  }
  assert_eq!(
    hex_digest(&fs::read(src.join("HTMLparser.c")).unwrap()),
    "9330d72ee2926aeb48516184fbbcfc75dc840f096edf7629b66eb040eeb9723c",
    "patched source drift"
  );
  let on = [
    "HTML", "XPATH", "PUSH", "THREADS", "C14N", "OUTPUT", "PATTERN", "READER", "REGEXPS",
    "RELAXNG", "SAX1", "SCHEMAS", "VALID", "WRITER", "XPTR", "ISO8859X", "DEBUG", "ICONV",
  ];
  let off = [
    "PYTHON",
    "PROGRAMS",
    "TESTS",
    "DOCS",
    "HTTP",
    "MODULES",
    "CATALOG",
    "ICU",
    "ZLIB",
    "XINCLUDE",
    "READLINE",
    "LEGACY",
    "SCHEMATRON",
    "THREAD_ALLOC",
    "TLS",
  ];
  let mut configure = Command::new("cmake");
  configure
    .arg("-S")
    .arg(&src)
    .arg("-B")
    .arg(&build)
    .arg(format!("-DCMAKE_INSTALL_PREFIX={}", prefix.display()))
    .args([
      "-DBUILD_SHARED_LIBS=OFF",
      "-DCMAKE_POSITION_INDEPENDENT_CODE=ON",
      "-DCMAKE_BUILD_TYPE=Release",
      "-DCMAKE_EXPORT_COMPILE_COMMANDS=ON",
    ]);
  let sdk = if target.ends_with("-apple-darwin") {
    let output = Command::new("xcrun")
      .args(["--sdk", "macosx", "--show-sdk-path"])
      .output()
      .expect("macOS SDK required");
    assert!(output.status.success());
    let sdk = PathBuf::from(String::from_utf8(output.stdout).unwrap().trim());
    configure
      .arg(format!(
        "-DIconv_INCLUDE_DIR={}",
        sdk.join("usr/include").display()
      ))
      .arg(format!(
        "-DIconv_LIBRARY={}",
        sdk.join("usr/lib/libiconv.tbd").display()
      ));
    Some(sdk)
  } else {
    None
  };
  for feature in on {
    configure.arg(format!("-DLIBXML2_WITH_{feature}=ON"));
  }
  for feature in off {
    configure.arg(format!("-DLIBXML2_WITH_{feature}=OFF"));
  }
  run(&mut configure);
  let cache = fs::read_to_string(build.join("CMakeCache.txt")).unwrap();
  if let Some(sdk) = sdk {
    assert!(
      cache.lines().any(|line| line
        == format!(
          "Iconv_LIBRARY:FILEPATH={}",
          sdk.join("usr/lib/libiconv.tbd").display()
        )),
      "unexpected iconv library"
    );
    assert!(
      cache.lines().any(|line| line
        == format!(
          "Iconv_INCLUDE_DIR:PATH={}",
          sdk.join("usr/include").display()
        )),
      "unexpected iconv headers"
    );
  } else {
    assert!(
      cache
        .lines()
        .any(|line| line == "Iconv_IS_BUILT_IN:INTERNAL=1"),
      "GNU backend requires glibc builtin iconv; no external fallback"
    );
  }
  fs::copy(
    build.join("CMakeCache.txt"),
    out.join("a05-native-feature-receipt.txt"),
  )
  .unwrap();
  run(
    Command::new("cmake")
      .arg("--build")
      .arg(&build)
      .args(["--parallel", "2"]),
  );
  run(Command::new("cmake").arg("--install").arg(&build));
  // Prove pkg-config sees this exact vendor prefix, never ambient host metadata.
  for (arg, expected) in [
    ("--modversion", "2.15.4".to_string()),
    ("--variable=prefix", prefix.to_string_lossy().into_owned()),
  ] {
    let output = Command::new("pkg-config")
      .env("PKG_CONFIG_PATH", prefix.join("lib/pkgconfig"))
      .env("PKG_CONFIG_LIBDIR", prefix.join("lib/pkgconfig"))
      .arg(arg)
      .arg("libxml-2.0")
      .output()
      .expect("pkg-config required");
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), expected);
  }
  assert!(prefix.join("lib/libxml2.a").is_file());
  println!(
    "cargo:rustc-link-search=native={}",
    prefix.join("lib").display()
  );
  println!("cargo:rustc-link-lib=static=xml2");
  if target.ends_with("-unknown-linux-gnu") {
    println!("cargo:rustc-link-lib=m");
    println!("cargo:rustc-link-lib=pthread");
  }
  if target.ends_with("-apple-darwin") {
    println!("cargo:rustc-link-lib=iconv");
  }
  println!("cargo:rerun-if-changed={}", patch.display());
  let bindings = bindgen::Builder::default()
    .header("src/wrapper.h")
    .opaque_type("max_align_t")
    .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
    .layout_tests(true)
    .clang_arg(format!("-I{}", prefix.join("include/libxml2").display()))
    .generate()
    .expect("vendor headers/bindings must agree");
  bindings.write_to_file(out.join("bindings.rs")).unwrap();
  println!("cargo::rustc-check-cfg=cfg(libxml_older_than_2_12)");
  println!("cargo:rerun-if-changed={}", archive.display());
  println!("cargo:rerun-if-changed=build.rs");
  println!("cargo:rerun-if-changed=src/wrapper.h");
  println!("cargo:rerun-if-env-changed=LIBXML2");
}
