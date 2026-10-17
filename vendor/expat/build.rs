//! Offline native candidate; no system Expat discovery or network fallback.
use sha2::{Digest, Sha256};
use std::{env, fs, path::PathBuf, process::Command};
fn run(c: &mut Command) {
    assert!(
        c.status().expect("native build tool required").success(),
        "native build failed: {c:?}"
    )
}
fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let archive = manifest.join("../expat-source/expat-2.9.0.tar.xz");
    let digest = Sha256::digest(fs::read(&archive).expect("vendored Expat required; no fallback"))
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    assert_eq!(
        digest, "1e6371862cc31999b368c3b89b49994f0677e1bab5f1b2b85ae3741f5d803051",
        "Expat archive drift"
    );
    let host = env::var("HOST").unwrap();
    let target = env::var("TARGET").unwrap();
    assert_eq!(host, target, "cross build unqualified");
    assert!(
        target == "aarch64-apple-darwin"
            || target == "x86_64-apple-darwin"
            || target == "aarch64-unknown-linux-gnu"
            || target == "x86_64-unknown-linux-gnu",
        "unqualified Expat target"
    );
    for name in [
        "EXPAT_DIR",
        "EXPAT_LIB_DIR",
        "EXPAT_INCLUDE_DIR",
        "EXPAT_STATIC",
        "CMAKE_TOOLCHAIN_FILE",
    ] {
        assert!(
            env::var_os(name).is_none(),
            "host Expat/build override forbidden: {name}"
        )
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let source = out.join("expat-2.9.0");
    let build = out.join("native-build");
    if !source.is_dir() {
        run(Command::new("tar")
            .arg("-xJf")
            .arg(&archive)
            .arg("-C")
            .arg(&out));
    }
    run(Command::new("cmake")
        .arg("-S")
        .arg(&source)
        .arg("-B")
        .arg(&build)
        .args([
            "-DCMAKE_BUILD_TYPE=Release",
            "-DCMAKE_POSITION_INDEPENDENT_CODE=ON",
            "-DEXPAT_SHARED_LIBS=OFF",
            "-DEXPAT_BUILD_TOOLS=OFF",
            "-DEXPAT_BUILD_EXAMPLES=OFF",
            "-DEXPAT_BUILD_TESTS=OFF",
            "-DEXPAT_BUILD_DOCS=OFF",
            "-DEXPAT_BUILD_PKGCONFIG=OFF",
            "-DEXPAT_DTD=ON",
            "-DEXPAT_GE=ON",
            "-DEXPAT_NS=ON",
            "-DEXPAT_CHAR_TYPE=char",
            "-DEXPAT_CONTEXT_BYTES=1024",
        ]));
    run(Command::new("cmake")
        .arg("--build")
        .arg(&build)
        .args(["--parallel", "2"]));
    let object = out.join("wrapper.o");
    run(Command::new("cc")
        .args([
            "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-fPIC", "-I",
        ])
        .arg(source.join("lib"))
        .arg("-c")
        .arg(manifest.join("wrapper.c"))
        .arg("-o")
        .arg(&object));
    let lib = out.join("libssx_expat_wrapper.a");
    run(Command::new("ar").arg("crs").arg(&lib).arg(object));
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-search=native={}", build.display());
    println!("cargo:rustc-link-lib=static=ssx_expat_wrapper");
    println!("cargo:rustc-link-lib=static=expat");
    println!("cargo:rerun-if-changed={}", archive.display());
    println!("cargo:rerun-if-changed=wrapper.c");
    println!("cargo:rerun-if-changed=build.rs");
}
