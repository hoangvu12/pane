//! Building Pane's packages and the artifacts its default extensions
//! are acquired from (#53 for Linux, #51 for Windows).
//!
//! `package-linux` and `package-windows` each produce, under
//! `target/dist/`:
//!
//! - `pane-<version>-<os>-<arch>[-dev].tar.gz` (Linux) or `.zip`
//!   (Windows) — the package a clean machine of that system installs
//!   from: the `pane` program, the install script and a README (and, on
//!   Linux, a desktop entry), and none of the default extensions'
//!   payloads (internet-first: Pane downloads them at first setup). A
//!   `.sha256` file beside it names its digest; nothing is signed, since
//!   no signing credentials exist yet.
//! - `artifacts/` — what an artifact source serves: the index document
//!   `pane-defaults.json` and one tarball per default extension's payload,
//!   built for the system this ran on. A real deployment serves this
//!   folder at Pane's published downloads; the tests and smokes serve it
//!   from this computer (`scripts/artifact_server.py`) instead, so no
//!   check reaches the network.
//!
//! `--dev` builds the package's program in the development profile: the
//! native smokes install that one, because only a development build takes
//! its artifact source from `PANE_ARTIFACTS` (a release build uses Pane's
//! published downloads, which no controlled source may replace). Without
//! it, the release profile is built.
//!
//! Each task builds the package for the system it runs on, so a release
//! for several systems builds one package per system (this machine builds
//! the Linux one, CI's `windows-2025` and `ubuntu-24.04` runners theirs).
//! `package-windows` runs everywhere far enough to assemble the
//! artifacts, then refuses anywhere but Windows: `pane.exe` needs a
//! Windows build, and packing another system's program under a Windows
//! package's name would be worse than explaining so.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::PACKED_MTIME;

use serde_json::{Value, json};
use sha2::{Digest, Sha256, Sha512};

use crate::zip;

/// The default extensions whose payloads the artifacts describe, and the
/// assembled package each is packed from: the calculator (the default
/// feature the installer proves) and the helper sample (the prebuilt
/// helper fixture with it). The ids are the ones Pane's application build
/// acquires (`pane::default_extensions`).
const DEFAULTS: [(&str, &str); 2] = [
    ("calculator", "calculator"),
    ("helper-sample", "sample-helper"),
];

/// The pane program's version, as the package names it: this workspace's
/// version, which every crate of it shares.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Which system one of the package tasks builds for.
#[derive(Clone, Copy)]
enum System {
    Linux,
    Windows,
}

impl System {
    /// The system's id, as the package's name and the docs say it.
    fn id(self) -> &'static str {
        match self {
            System::Linux => "linux",
            System::Windows => "windows",
        }
    }

    /// The install script the package holds: where the repository keeps it,
    /// and the name it is packed under.
    fn install_script(self) -> (&'static str, &'static str) {
        match self {
            System::Linux => ("scripts/install-linux.sh", "install.sh"),
            System::Windows => ("scripts/install-windows.ps1", "install.ps1"),
        }
    }

    /// How the package is packed: the tarball Linux unpacks with `tar`, or
    /// the zip a Windows user unzips with whatever is at hand (Windows has
    /// no tar a user can rely on).
    fn archive(self) -> &'static str {
        match self {
            System::Linux => "tar.gz",
            System::Windows => "zip",
        }
    }

    /// The one file this system's package holds besides the program, the
    /// install script and the README: Linux's desktop entry, which Windows'
    /// install script replaces with a Start-menu shortcut.
    fn extra_file(self) -> Option<(&'static str, String)> {
        match self {
            System::Linux => Some(("pane.desktop", desktop_entry())),
            System::Windows => None,
        }
    }
}

/// Builds the Linux package and the default extensions' artifacts.
pub fn linux(dev: bool) -> Result<(), String> {
    let root = root();
    let out = root.join("target/dist");
    fs::create_dir_all(&out).map_err(|error| error.to_string())?;
    build_program(&root, dev)?;
    let artifacts = assemble_artifacts(&root, &out)?;
    let package = assemble_package(&root, &out, dev, System::Linux)?;
    println!("package built into {}", package.display());
    println!("artifacts built into {}", artifacts.display());
    Ok(())
}

/// Builds the Windows package and the default extensions' artifacts.
pub fn windows(dev: bool) -> Result<(), String> {
    let root = root();
    let out = root.join("target/dist");
    fs::create_dir_all(&out).map_err(|error| error.to_string())?;
    let artifacts = assemble_artifacts(&root, &out)?;
    // The program comes last, so everything else the task builds is built
    // wherever it runs; but pane.exe can only be built by a Windows
    // checkout (no cross toolchain is set up: a Windows program needs a
    // Windows build), and packing another system's program under a
    // Windows package's name would be worse than explaining so. CI's
    // `windows-2025` runner builds the package itself.
    if !cfg!(target_os = "windows") {
        return Err(format!(
            "package-windows builds the pane program for Windows, which only a Windows checkout \
             can build; this one runs on {}. The artifacts under {} are assembled for this \
             system, and a Windows run re-assembles them for windows-x86_64",
            std::env::consts::OS,
            artifacts.display()
        ));
    }
    build_program(&root, dev)?;
    let package = assemble_package(&root, &out, dev, System::Windows)?;
    println!("package built into {}", package.display());
    println!("artifacts built into {}", artifacts.display());
    Ok(())
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn cargo() -> Command {
    Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
}

fn run(command: &mut Command) -> Result<(), String> {
    let status = command
        .status()
        .map_err(|error| format!("failed to start {command:?}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{command:?} failed with {status}"))
    }
}

/// Builds the `pane` program for the system this runs on.
fn build_program(root: &Path, dev: bool) -> Result<(), String> {
    let mut build = cargo();
    build
        .current_dir(root)
        .args(["build", "--locked", "-p", "pane"]);
    if !dev {
        build.arg("--release");
    }
    run(&mut build)
}

/// Where `cargo build` put the `pane` program.
fn program(root: &Path, dev: bool) -> PathBuf {
    let profile = if dev { "debug" } else { "release" };
    root.join("target").join(profile).join(program_name())
}

fn program_name() -> String {
    format!("pane{}", std::env::consts::EXE_SUFFIX)
}

/// The system this runs on, as Pane names a helper target (`linux-x86_64`).
fn target_id() -> Result<String, String> {
    pane_target::Target::current()
        .map(|target| target.id())
        .ok_or_else(|| "Pane names no target for this system".to_owned())
}

/// Assembles the artifacts an artifact source serves into
/// `target/dist/artifacts`: the index `pane-defaults.json` and one tarball
/// per default extension's payload, packed from the package `cargo xtask
/// guests` assembled. The payload's manifest names the helper targets
/// whose files it carries: the build serves the helper built for the
/// system it ran on, so the manifest is rewritten to name that target
/// alone (a real deployment builds every supported target and serves one
/// payload whose manifest names them all).
fn assemble_artifacts(root: &Path, out: &Path) -> Result<PathBuf, String> {
    let artifacts = out.join("artifacts");
    let _ = fs::remove_dir_all(&artifacts);
    fs::create_dir_all(&artifacts).map_err(|error| error.to_string())?;
    let mut entries: Vec<String> = Vec::new();
    for (id, package) in DEFAULTS {
        let source = root.join("target/guests/packages").join(package);
        if !source.is_dir() {
            return Err(format!(
                "{} is missing; run `cargo xtask guests` first",
                source.display()
            ));
        }
        let files = payload_files(&source, id)?;
        let version = manifest_version(&files)?;
        let file = format!("{id}-{version}.tgz");
        let tarball = pack(&files);
        let integrity = format!("sha512-{}", base64(&Sha512::digest(&tarball)));
        fs::write(artifacts.join(&file), &tarball).map_err(|error| {
            format!("write {} failed: {error}", artifacts.join(&file).display())
        })?;
        entries.push(format!(
            "  {{ \"id\": \"{id}\", \"version\": \"{version}\", \"file\": \"{file}\", \"integrity\": \"{integrity}\", \"size\": {} }}",
            tarball.len()
        ));
    }
    let index = format!(
        "{{\n  \"formatVersion\": 1,\n  \"defaults\": [\n{}\n  ]\n}}\n",
        entries.join(",\n")
    );
    let index_file = artifacts.join("pane-defaults.json");
    fs::write(&index_file, index)
        .map_err(|error| format!("write {} failed: {error}", index_file.display()))?;
    Ok(artifacts)
}

/// The files of the payload packed from `source`: the manifest, and the
/// files it names — the components of its commands and the helper file
/// for this system — exactly what Pane installs from it. The helper
/// sample's manifest is rewritten to name this system's helper target
/// alone, since the build assembles the helper for the system it runs on;
/// nothing else in the assembled folder is packed, so a file a helper run
/// left beside its program never travels.
fn payload_files(source: &Path, _id: &str) -> Result<Vec<(String, Vec<u8>)>, String> {
    let assembled = read_files(source, "")?;
    let read = |path: &str| {
        assembled
            .iter()
            .find(|(name, _)| name == path)
            .map(|(_, contents)| contents.clone())
            .ok_or_else(|| {
                format!(
                    "{path} is missing from the assembled package {}",
                    source.display()
                )
            })
    };
    let mut manifest: Value = serde_json::from_slice(&read("pane.json")?)
        .map_err(|error| format!("the payload's pane.json cannot be read: {error}"))?;
    // The files the manifest names, which Pane installs from the payload.
    let mut named: Vec<String> = Vec::new();
    if let Some(commands) = manifest["commands"].as_array() {
        for command in commands {
            let component = command["component"].as_str().expect("a component");
            named.push(component.to_owned());
        }
    }
    let target = target_id()?;
    if let Some(helpers) = manifest["helpers"].as_array_mut() {
        // The build assembles the helper for the system it runs on, so the
        // payload names that target alone.
        let file = format!("helpers/{target}/pane-echo{}", exe_suffix());
        if read(&file).is_err() {
            return Err(format!(
                "the helper sample ships no {file}; `cargo xtask guests` builds it for the \
                 system it runs on"
            ));
        }
        for helper in helpers {
            helper["targets"] = json!({ target.clone(): file.clone() });
        }
        named.push(file);
    }
    let manifest_text = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| format!("the rewritten pane.json cannot be written: {error}"))?;
    let mut payload = vec![("pane.json".to_owned(), manifest_text)];
    payload.extend(
        named
            .into_iter()
            .map(|path| Ok((path.clone(), read(&path)?)))
            .collect::<Result<Vec<_>, String>>()?,
    );
    Ok(payload)
}

fn exe_suffix() -> &'static str {
    if cfg!(windows) { ".exe" } else { "" }
}

/// Every file under `folder`, by its path relative to it, in name order.
fn read_files(folder: &Path, prefix: &str) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut files = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(folder)
        .map_err(|error| format!("read {} failed: {error}", folder.display()))?
        .map(|entry| entry.map_err(|error| error.to_string()))
        .collect::<Result<_, String>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| format!("{} holds a file name that is not UTF-8", folder.display()))?;
        let in_package = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        if path.is_dir() {
            files.extend(read_files(&path, &in_package)?);
        } else {
            files.push((
                in_package,
                fs::read(&path).map_err(|error| error.to_string())?,
            ));
        }
    }
    Ok(files)
}

/// The version a payload's `pane.json` declares.
fn manifest_version(files: &[(String, Vec<u8>)]) -> Result<String, String> {
    let manifest = files
        .iter()
        .find(|(path, _)| path == "pane.json")
        .map(|(_, contents)| contents.clone())
        .ok_or("the payload has no pane.json")?;
    let manifest: Value = serde_json::from_slice(&manifest)
        .map_err(|error| format!("the payload's pane.json cannot be read: {error}"))?;
    manifest["version"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| "the payload's pane.json declares no version".to_owned())
}

/// Packs `files` (path in the package, contents) as Pane's own downloads
/// pack a payload: a gzip of a tar holding them under `package/`, every
/// file a regular file without execute permission, with the fixed time,
/// owner and mode that make the tarball the same on every system.
fn pack(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    pack_tgz(files, "package", None).expect("packing into memory")
}

/// Packs `files` (path in the archive, contents) as the tarballs this
/// repository packs: a gzip of a tar holding them under `prefix/`, every
/// file a regular file, `program` (when given) executable, with the fixed
/// time, owner and mode that make the bytes the same on every system.
fn pack_tgz(
    files: &[(String, Vec<u8>)],
    prefix: &str,
    program: Option<&str>,
) -> Result<Vec<u8>, String> {
    let mut tar = tar::Builder::new(Vec::new());
    for (path, contents) in files {
        let mode = if Some(path.as_str()) == program {
            0o755
        } else {
            0o644
        };
        let mut header = tar::Header::new_ustar();
        header.set_size(contents.len() as u64);
        header.set_mode(mode);
        header.set_mtime(PACKED_MTIME);
        header.set_uid(0);
        header.set_gid(0);
        header.set_entry_type(tar::EntryType::Regular);
        tar.append_data(&mut header, format!("{prefix}/{path}"), contents.as_slice())
            .map_err(|error| format!("packing {path} failed: {error}"))?;
    }
    let tar = tar.into_inner().map_err(|error| error.to_string())?;
    let mut gz = flate2::GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), flate2::Compression::best());
    std::io::Write::write_all(&mut gz, &tar).map_err(|error| error.to_string())?;
    gz.finish().map_err(|error| error.to_string())
}

/// Assembles the package: the `pane` program, the install script, a
/// README (and, on Linux, a desktop entry) in `pane/`, packed as
/// `pane-<version>-<os>-<arch>[-dev].tar.gz` on Linux and
/// `pane-<version>-<os>-<arch>[-dev].zip` on Windows, with a `.sha256`
/// file beside it. The package is the same bytes wherever it is built: the
/// tarball with fixed time, owner and mode, the zip the same way
/// ([`zip::pack`]).
fn assemble_package(root: &Path, out: &Path, dev: bool, system: System) -> Result<PathBuf, String> {
    let stage = out.join("stage/pane");
    let _ = fs::remove_dir_all(stage.parent().expect("the stage folder"));
    fs::create_dir_all(&stage).map_err(|error| error.to_string())?;
    let suffix = if dev { "-dev" } else { "" };
    let name = format!(
        "pane-{VERSION}-{}-{}{suffix}.{}",
        system.id(),
        arch().ok_or("the package names no architecture for this system")?,
        system.archive(),
    );
    let (script, script_name) = system.install_script();
    let copies = [
        (program(root, dev), stage.join(program_name())),
        (root.join(script), stage.join(script_name)),
    ];
    for (from, to) in copies {
        fs::copy(&from, &to).map_err(|error| format!("copy {} failed: {error}", from.display()))?;
    }
    fs::write(stage.join("README.txt"), readme(system, dev)).map_err(|error| error.to_string())?;
    if let Some((file, contents)) = system.extra_file() {
        fs::write(stage.join(file), contents).map_err(|error| error.to_string())?;
    }
    let files = read_files(&stage, "")?;
    let packed = match system {
        System::Linux => pack_tgz(&files, "pane", Some(&program_name()))?,
        System::Windows => zip::pack(&files, "pane")?,
    };
    let package = out.join(&name);
    fs::write(&package, &packed)
        .map_err(|error| format!("write {} failed: {error}", package.display()))?;
    // As `sha256sum` writes it, so the file can be checked with
    // `sha256sum -c` on Linux; on Windows, `Get-FileHash` prints the digest
    // to compare with.
    let digest: String = Sha256::digest(&packed)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let sha256 = out.join(format!("{name}.sha256"));
    fs::write(&sha256, format!("{digest}  {name}\n"))
        .map_err(|error| format!("write {} failed: {error}", sha256.display()))?;
    Ok(package)
}

/// The architecture of the system this ran on, as the package name says
/// it.
fn arch() -> Option<&'static str> {
    match std::env::consts::ARCH {
        "x86_64" => Some("x86_64"),
        "aarch64" => Some("aarch64"),
        _ => None,
    }
}

/// The README in the package.
fn readme(system: System, dev: bool) -> String {
    match system {
        System::Linux => readme_linux(dev),
        System::Windows => readme_windows(dev),
    }
}

fn readme_linux(dev: bool) -> String {
    let profile = if dev { "development" } else { "release" };
    format!(
        "Pane {VERSION} for Linux (this package is the {profile} profile)

WHAT THIS IS

  Pane, a desktop launcher. This package holds the pane program and
  installs it for one user; it holds none of Pane's default extensions:
  Pane downloads them itself the first time it runs, from Pane's own
  downloads (https://downloads.pane.sh/).

PREREQUISITES

  A 64-bit Linux with X11, and these libraries (Ubuntu 24.04 package
  names; the install script checks what is missing):

    libxkbcommon0 libxkbcommon-x11-0 libwayland-client0 libxcb1
    libfontconfig1 libfreetype6 libvulkan1

  plus a Vulkan driver; Mesa's mesa-vulkan-drivers works without a GPU.

INSTALL

  bash install.sh            # installs to ~/.local
  bash install.sh --prefix X # installs to X instead

  It copies the pane program to <prefix>/bin and a desktop entry to
  <prefix>/share/applications, and runs `pane --version` to check what it
  installed. No root is needed.

UNINSTALL

  Remove <prefix>/bin/pane and <prefix>/share/applications/pane.desktop.
  Pane keeps its own data in ~/.local/share/pane (its installed
  extensions and their settings); remove that folder to remove them too.

FIRST RUN

  The first run downloads Pane's default extensions (the calculator) from
  https://downloads.pane.sh/ and shows their progress; Pane stays usable
  if the download fails, and offers to try again. That location is not
  deployed yet, so today a first run on the real internet explains that
  it cannot reach it and keeps everything else working.

  NOTHING IS SIGNED

  The package is not signed: no signing credentials exist. Its sha256 is
  in the pane-<version>-linux-<arch>.tar.gz.sha256 file beside it, which
  says only what was packed.
"
    )
}

fn readme_windows(dev: bool) -> String {
    let profile = if dev { "development" } else { "release" };
    format!(
        r#"Pane {VERSION} for Windows (this package is the {profile} profile)

WHAT THIS IS

  Pane, a desktop launcher. This package holds the pane.exe program and
  installs it for one user; it holds none of Pane's default extensions:
  Pane downloads them itself the first time it runs, from Pane's own
  downloads (https://downloads.pane.sh/).

PREREQUISITES

  A 64-bit x86_64 Windows. The system this package was built and checked
  on is Windows Server 2025 (CI's windows-2025 runner); no other Windows
  has been tried. Nothing else is needed: no Node, Rust, npm, Git or
  compiler, and no administrator rights.

INSTALL

  Unzip this package (Explorer unzips it, or Expand-Archive in
  PowerShell), then, in the pane folder it unpacked:

    powershell -ExecutionPolicy Bypass -File install.ps1

  It copies the pane.exe program to %LOCALAPPDATA%\Pane, puts a Pane
  shortcut in your Start menu, and runs `pane --version` to check what it
  installed. No administrator rights are needed. To install somewhere
  else, add -InstallDir <folder>; to install from another folder, add
  -PackageFolder <folder>.

  PowerShell may refuse install.ps1 as a script that came from the
  internet (nothing here is signed): the -ExecutionPolicy Bypass above
  answers that, or run Unblock-File install.ps1 once. Check the package's
  digest against the pane-<version>-windows-<arch>.zip.sha256 file beside
  it if it reached you over the internet.

UNINSTALL

  Remove %LOCALAPPDATA%\Pane\pane.exe and the Pane shortcut in your Start
  menu (close Pane first). Pane keeps its own data in
  %LOCALAPPDATA%\Pane\data (its installed extensions and their settings);
  remove that folder to remove them too.

FIRST RUN

  The first run downloads Pane's default extensions (the calculator) from
  https://downloads.pane.sh/ and shows their progress; Pane stays usable
  if the download fails, and offers to try again. That location is not
  deployed yet, so today a first run on the real internet explains that
  it cannot reach it and keeps everything else working.

  NOTHING IS SIGNED

  The package is not signed: no signing credentials exist (no
  Authenticode certificate), so Windows may warn about an unknown
  publisher when the program or the install script runs. Its sha256 is in
  the pane-<version>-windows-<arch>.zip.sha256 file beside it, which says
  only what was packed."#
    )
}

/// The desktop entry in the package.
fn desktop_entry() -> String {
    "[Desktop Entry]\n\
Type=Application\n\
Name=Pane\n\
GenericName=Launcher\n\
Comment=Launch applications, calculate and run extension commands\n\
Exec=pane\n\
Terminal=false\n\
Categories=Utility;\n"
        .to_owned()
}

/// Standard base64 with padding.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut text = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if i <= chunk.len() {
                text.push(ALPHABET[(n >> shift) as usize & 63] as char);
            } else {
                text.push('=');
            }
        }
    }
    text
}
