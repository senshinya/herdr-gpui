// Go is a build-time tool; app child processes still use herdr_client::new_command.
#![allow(clippy::disallowed_methods)]
use std::{env, error::Error, fs, path::PathBuf, process::Command};

fn main() -> Result<(), Box<dyn Error>> {
    let manifest =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").ok_or("missing manifest directory")?);
    let source = manifest.join("../../native/tailscale");
    let out = PathBuf::from(env::var_os("OUT_DIR").ok_or("missing build directory")?);
    let target = env::var("TARGET")?;
    let host = env::var("HOST")?;
    for name in [
        "bridge.h",
        "engine.go",
        "ffi.go",
        "inbound.go",
        "registry.go",
        "status.go",
        "go.mod",
        "go.sum",
    ] {
        println!("cargo:rerun-if-changed={}", source.join(name).display());
    }
    for name in ["CC", "CGO_CFLAGS", "CGO_LDFLAGS", "GOTOOLCHAIN", "GOROOT"] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    let (goos, mode, name) = if target.contains("windows") {
        ("windows", "c-shared", "herdr_tailscale.dll")
    } else if target.contains("apple-darwin") {
        ("darwin", "c-archive", "libherdr_tailscale.a")
    } else if target.contains("linux") {
        ("linux", "c-archive", "libherdr_tailscale.a")
    } else {
        return Err(format!("Tailscale embedding does not support {target}").into());
    };
    let arch = if target.starts_with("aarch64") {
        "arm64"
    } else if target.starts_with("x86_64") {
        "amd64"
    } else {
        return Err(format!("Tailscale embedding does not support {target}").into());
    };
    let version = Command::new("go")
        .env("GOTOOLCHAIN", "local")
        .args(["env", "GOVERSION"])
        .output()?;
    if !version.status.success() || String::from_utf8_lossy(&version.stdout).trim() != "go1.26.7" {
        return Err("embedded Tailscale requires the pinned Go 1.26.7 toolchain".into());
    }
    let mut command = Command::new("go");
    command
        .current_dir(&source)
        .env("GOTOOLCHAIN", "local")
        .env("CGO_ENABLED", "1")
        .env("GOOS", goos)
        .env("GOARCH", arch)
        .args(["build", "-mod=readonly", "-trimpath"])
        .arg(format!("-buildmode={mode}"))
        .arg("-o")
        .arg(out.join(name));
    let cc_key = format!("CC_{}", target.replace('-', "_"));
    println!("cargo:rerun-if-env-changed={cc_key}");
    if let Some(cc) = env::var_os(&cc_key).or_else(|| env::var_os("CC")) {
        command.env("CC", cc);
    } else if goos == "windows" {
        command.env(
            "CC",
            format!(
                "{}-w64-mingw32-clang",
                if arch == "amd64" { "x86_64" } else { "aarch64" }
            ),
        );
    } else if host != target && goos == "darwin" {
        command.env(
            "CC",
            format!(
                "clang -arch {}",
                if arch == "amd64" { "x86_64" } else { "arm64" }
            ),
        );
    }
    let result = command.status()?;
    if !result.success() {
        return Err(
            "building embedded Tailscale failed (Go and the target's C compiler are required)"
                .into(),
        );
    }
    println!("cargo:rustc-link-search=native={}", out.display());
    if goos == "darwin" {
        for framework in ["CoreFoundation", "Security"] {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
        println!("cargo:rustc-link-lib=resolv");
    }
    if goos == "windows" {
        // The executable, test binaries and app packaging all load the same pinned ABI.
        let profile = out
            .ancestors()
            .nth(3)
            .ok_or("missing Cargo profile directory")?;
        for directory in [profile.to_path_buf(), profile.join("deps")] {
            fs::create_dir_all(&directory)?;
            fs::copy(out.join(name), directory.join(name))?;
        }
    }
    Ok(())
}
