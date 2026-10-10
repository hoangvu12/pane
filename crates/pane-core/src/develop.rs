//! Development mode's builds: how a local package's source folder is built
//! on save (ADR 0004; #12, #13). What a build is, and the session that runs
//! one after each save, are the `pane-build` crate's, which `pane-ext`
//! builds with too (ADR 0047, #216); this module gives it Pane's manifest
//! and resolves what `pane-ext dev` develops ([`target`]): the package in a
//! folder, or one extension of the collection at it, named by its id
//! (ADR 0044).

use std::path::{Path, PathBuf};

pub use pane_build::{Build, BuildJob, BuildOutcome, Builder};

use crate::packages::{Manifest, PackageIdentity};

/// Pane's builds, reading a package's manifest as Pane does.
pub type Toolchains = pane_build::Toolchains<PaneManifest>;

/// What a package's `pane.json` names, read by Pane's own rules.
#[derive(Clone, Copy, Debug, Default)]
pub struct PaneManifest;

impl pane_build::ManifestFiles for PaneManifest {
    fn components(&self, folder: &Path) -> Result<Vec<PathBuf>, String> {
        let (manifest, _) = Manifest::read_parsed(folder).map_err(|error| error.to_string())?;
        let mut components: Vec<PathBuf> = Vec::new();
        for (_, component) in manifest.components() {
            if !components.iter().any(|known| known == component) {
                components.push(component.to_path_buf());
            }
        }
        Ok(components)
    }

    fn helper_files(&self, folder: &Path) -> Result<Vec<PathBuf>, String> {
        let (manifest, _) = Manifest::read_parsed(folder).map_err(|error| error.to_string())?;
        Ok(manifest
            .helpers
            .iter()
            .filter_map(|helper| helper.for_this_system())
            .map(Path::to_path_buf)
            .collect())
    }
}

/// The package `pane-ext dev` develops in the folder its argument names
/// ([`target`]): the package in a folder, or one extension of the
/// collection at it, named by its id after `#` (ADR 0044) — developed
/// alone, as the package it is, so the collection's other extensions are
/// never touched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DevTarget {
    /// The identity of the package, as Pane installs and develops it: the
    /// collection's resolved folder with the id, for one extension of a
    /// collection.
    pub identity: PackageIdentity,
    /// The folder the package is built and previewed in: the extension's,
    /// of a collection, else the package folder itself.
    pub folder: PathBuf,
    /// The collection's resolved folder and the id of the extension of it
    /// that is developed — `<folder>#<id>` names the package, as `pane
    /// --install` and `pane-ext dev` take it. `None` for a package
    /// folder, which its own folder names.
    pub collection: Option<(PathBuf, String)>,
}

/// The package `pane-ext dev` develops at `folder`, where `id` names one
/// extension of the collection at `folder` (ADR 0044) and `None` the
/// package folder itself: one extension of a collection is built in its
/// own folder, with the identity of the collection's folder and the id,
/// and a collection named without an id is explained, as an install
/// names one (the refusals an install of one extension meets, #307).
/// The package's manifest is not read: `pane-ext` checks it holds one,
/// and a build is what is checked.
pub fn target(folder: &Path, id: Option<&str>) -> Result<DevTarget, String> {
    let Some(id) = id else {
        // A collection named without an id is explained; a folder holding
        // both files is refused, as `SourcePackage::read` refuses it.
        let file = crate::collections::COLLECTION_FILE;
        let invalid = |why: String| {
            format!(
                "The folder {} is a collection whose {} is invalid: {why}",
                folder.display(),
                file
            )
        };
        return match crate::collections::read(folder).map_err(invalid)? {
            Some(_) if folder.join(pane_build::MANIFEST_FILE).is_file() => Err(format!(
                "The folder {} holds both {} and {file}: a folder is one extension or a \
                 collection, never both",
                folder.display(),
                pane_build::MANIFEST_FILE
            )),
            Some(_) => Err(format!(
                "The folder {} is a collection, not one extension: its root holds {file}, which \
                 lists the extensions it offers by id; name the one to develop after `#`, as \
                 {}#<id>",
                folder.display(),
                folder.display()
            )),
            None => {
                let identity = PackageIdentity::local(folder).map_err(|error| error.to_string())?;
                let folder = identity
                    .local_folder()
                    .expect("a local identity has a folder")
                    .to_path_buf();
                Ok(DevTarget {
                    identity,
                    folder,
                    collection: None,
                })
            }
        };
    };
    let (identity, root, folder) =
        crate::packages::collection_extension(folder, id).map_err(|error| error.to_string())?;
    Ok(DevTarget {
        identity,
        folder,
        collection: Some((root, id.to_owned())),
    })
}

#[cfg(test)]
mod tests {
    use pane_build::{is_save, stage_package};

    use super::*;

    fn manifest(folder: &Path, component: &str) {
        std::fs::write(
            folder.join("pane.json"),
            format!(
                r#"{{ "manifestVersion": 1, "title": "Hello", "apiVersion": "0.1",
                     "commands": [{{ "id": "hello", "title": "Hello", "component": "{component}" }}] }}"#
            ),
        )
        .unwrap();
    }

    fn toolchains() -> Toolchains {
        Toolchains {
            cargo: Some("cargo".into()),
            python: Some("python3".into()),
            componentize_js: Some(PathBuf::from("/pane/tools/componentize-js/pane_js.py")),
            manifests: PaneManifest,
        }
    }

    /// A collection of `clock` and `timers` at `root`, each extension a
    /// package of its own.
    fn collection(root: &Path) {
        for id in ["clock", "timers"] {
            let folder = root.join("extensions").join(id);
            std::fs::create_dir_all(&folder).unwrap();
            manifest(&folder, "command.wasm");
        }
        std::fs::write(
            root.join("pane-collection.json"),
            r#"{ "extensions": [ { "id": "clock", "path": "extensions/clock" },
                                 { "id": "timers", "path": "extensions/timers" } ] }"#,
        )
        .unwrap();
    }

    /// The folder as the identity spells it: resolved.
    fn resolved(folder: &Path) -> PathBuf {
        PackageIdentity::local(folder)
            .unwrap()
            .local_folder()
            .unwrap()
            .to_path_buf()
    }

    #[test]
    fn one_extension_of_a_collection_is_developed_in_its_own_folder() {
        let dir = tempfile::tempdir().unwrap();
        let root = resolved(&dir.path().join("tools"));
        collection(&root);
        let clock = target(&root, Some("clock")).unwrap();
        assert_eq!(
            clock.identity,
            PackageIdentity::local_extension(&root, "clock").unwrap()
        );
        assert_eq!(clock.folder, root.join("extensions/clock"));
        assert_eq!(clock.collection, Some((root.clone(), "clock".into())));
        // The collection's other extension has a target of its own, with
        // its own identity and folder.
        let timers = target(&root, Some("timers")).unwrap();
        assert_ne!(timers.identity, clock.identity);
        assert_eq!(timers.folder, root.join("extensions/timers"));
    }

    #[test]
    fn a_package_folder_is_developed_as_itself() {
        let dir = tempfile::tempdir().unwrap();
        manifest(dir.path(), "command.wasm");
        let identity = PackageIdentity::local(dir.path()).unwrap();
        let developed = target(dir.path(), None).unwrap();
        assert_eq!(developed.identity, identity);
        assert_eq!(developed.folder, resolved(dir.path()));
        assert_eq!(developed.collection, None);
    }

    #[test]
    fn a_collection_without_an_id_is_explained() {
        let dir = tempfile::tempdir().unwrap();
        let root = resolved(&dir.path().join("tools"));
        collection(&root);
        assert_eq!(
            target(&root, None).unwrap_err(),
            format!(
                "The folder {} is a collection, not one extension: its root holds \
                 pane-collection.json, which lists the extensions it offers by id; name the \
                 one to develop after `#`, as {}#<id>",
                root.display(),
                root.display()
            )
        );
    }

    #[test]
    fn every_way_a_name_cannot_be_developed_says_what_is_wrong() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("tools");
        collection(&root);
        // An id the collection does not list, and one that cannot be one.
        assert_eq!(
            target(&root, Some("nobody")).unwrap_err(),
            format!(
                "The collection at {} lists no extension `nobody` in its \
                 pane-collection.json",
                root.display()
            )
        );
        assert_eq!(
            target(&root, Some("Clock")).unwrap_err(),
            "the extension id `Clock` of a collection must be lowercase letters, digits and `-`"
        );
        // A path naming no package.
        std::fs::write(
            root.join("pane-collection.json"),
            r#"{ "extensions": [ { "id": "clock", "path": "extensions/none" } ] }"#,
        )
        .unwrap();
        assert_eq!(
            target(&root, Some("clock")).unwrap_err(),
            format!(
                "The collection at {} lists its extension `clock` at `extensions/none`, which \
                 is not a package: it has no pane.json",
                root.display()
            )
        );
        // A `#<id>` on a folder that is one extension, and on one that is
        // neither.
        let one = dir.path().join("one");
        manifest(&one, "command.wasm");
        assert_eq!(
            target(&one, Some("clock")).unwrap_err(),
            format!(
                "The folder {} is not a collection: its root holds pane.json, one extension, \
                 and no pane-collection.json; `#` names one extension of a collection",
                one.display()
            )
        );
        let neither = dir.path().join("neither");
        std::fs::create_dir(&neither).unwrap();
        assert_eq!(
            target(&neither, Some("clock")).unwrap_err(),
            format!(
                "The folder {} is not a collection: it holds no pane-collection.json; `#` \
                 names one extension of a collection",
                neither.display()
            )
        );
        // A folder holding both files, with or without an id.
        std::fs::copy(one.join("pane.json"), root.join("pane.json")).unwrap();
        for id in [None, Some("clock")] {
            let error = target(&root, id).unwrap_err();
            assert!(
                error.contains("holds both pane.json and pane-collection.json"),
                "{error}"
            );
        }
        // A folder that is not there.
        let missing = dir.path().join("missing");
        assert!(
            target(&missing, None)
                .unwrap_err()
                .starts_with("Cannot open")
        );
        // An index that cannot be taken.
        std::fs::write(root.join("pane-collection.json"), "{}").unwrap();
        std::fs::remove_file(root.join("pane.json")).unwrap();
        let error = target(&root, None).unwrap_err();
        assert!(
            error.contains("is a collection whose pane-collection.json is invalid"),
            "{error}"
        );
    }

    #[test]
    fn a_folder_with_cargo_toml_builds_with_cargo_and_ignores_its_output() {
        let folder = tempfile::tempdir().unwrap();
        manifest(folder.path(), "target/wasm32-wasip2/release/hello.wasm");
        std::fs::write(folder.path().join("Cargo.toml"), "").unwrap();
        let build = toolchains().build_for(folder.path()).unwrap();
        assert_eq!(
            build.command(),
            "cargo build --release --target wasm32-wasip2 --message-format=json-render-diagnostics"
        );
        for output in [
            "target",
            "target/wasm32-wasip2/release/hello.wasm",
            "Cargo.lock",
            "crates/inner/target/debug/x",
            "web/node_modules/zod/index.js",
            "web/dist/app.js",
        ] {
            assert!(!is_save(Path::new(output), &*build), "{output}");
        }
        for source in ["src/lib.rs", "Cargo.toml", "pane.json", "wit/world.wit"] {
            assert!(is_save(Path::new(source), &*build), "{source}");
        }
    }

    #[test]
    fn a_folder_with_package_json_builds_each_component_with_pane_js() {
        let folder = tempfile::tempdir().unwrap();
        manifest(folder.path(), "dist/hello.wasm");
        std::fs::write(folder.path().join("package.json"), "{}").unwrap();
        let build = toolchains().build_for(folder.path()).unwrap();
        let dir = folder.path().display();
        let out = folder.path().join("dist/hello.wasm");
        assert_eq!(
            build.command(),
            format!(
                "python3 /pane/tools/componentize-js/pane_js.py build {dir} {}",
                out.display()
            )
        );
        for output in ["dist", "dist/hello.wasm", "node_modules/zod/index.js"] {
            assert!(!is_save(Path::new(output), &*build), "{output}");
        }
        for source in ["src/index.ts", "package.json", "tsconfig.json", "pane.json"] {
            assert!(is_save(Path::new(source), &*build), "{source}");
        }
    }

    #[test]
    fn commands_quote_paths_with_spaces() {
        let folder = tempfile::tempdir().unwrap();
        let spaced = folder.path().join("my package");
        std::fs::create_dir(&spaced).unwrap();
        manifest(&spaced, "dist/hello.wasm");
        std::fs::write(spaced.join("package.json"), "{}").unwrap();
        let tools = Toolchains {
            python: Some("/opt/my python/bin/python3".into()),
            ..toolchains()
        };
        let command = tools.build_for(&spaced).unwrap().command();
        assert!(
            command.starts_with("\"/opt/my python/bin/python3\" "),
            "{command}"
        );
        assert!(
            command.contains(&format!("\"{}\"", spaced.display())),
            "{command}"
        );
    }

    #[test]
    fn hidden_files_and_editor_temporaries_are_not_saves() {
        let folder = tempfile::tempdir().unwrap();
        manifest(folder.path(), "hello.wasm");
        std::fs::write(folder.path().join("Cargo.toml"), "").unwrap();
        let build = toolchains().build_for(folder.path()).unwrap();
        for path in [
            ".git/index",
            "src/.lib.rs.swp",
            "src/lib.rs.swp",
            "src/4913",
            "src/lib.rs~",
            "src/lib.rs___jb_tmp___",
            "src/.#lib.rs",
            "src/#lib.rs#",
            "hello.wasm",
            "",
        ] {
            assert!(!is_save(Path::new(path), &*build), "{path}");
        }
    }

    #[test]
    fn a_folder_without_a_known_build_or_tool_is_explained() {
        let folder = tempfile::tempdir().unwrap();
        manifest(folder.path(), "hello.wasm");
        let error = toolchains().build_for(folder.path()).err().unwrap();
        assert!(error.contains("neither Cargo.toml"), "{error}");
        std::fs::write(folder.path().join("package.json"), "{}").unwrap();
        let no_js = Toolchains {
            componentize_js: None,
            ..toolchains()
        };
        let error = no_js.build_for(folder.path()).err().unwrap();
        assert!(error.contains("PANE_COMPONENTIZE_JS"), "{error}");
        let no_python = Toolchains {
            python: None,
            ..toolchains()
        };
        let error = no_python.build_for(folder.path()).err().unwrap();
        assert!(error.contains("PANE_PYTHON, then python3"), "{error}");
        std::fs::write(folder.path().join("Cargo.toml"), "").unwrap();
        let no_cargo = Toolchains {
            cargo: None,
            ..toolchains()
        };
        let error = no_cargo.build_for(folder.path()).err().unwrap();
        assert!(
            error.contains("cargo on PATH, then ~/.cargo/bin/cargo"),
            "{error}"
        );
    }

    #[test]
    fn staging_takes_pane_json_and_this_systems_helper_files() {
        let target = pane_target::Target::current().expect("Pane names this system's target");
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("source");
        let file = format!("helpers/tool{}", target.exe_suffix());
        let other = if target.id() == "linux-x86_64" {
            "linux-aarch64"
        } else {
            "linux-x86_64"
        };
        std::fs::create_dir_all(folder.join("helpers")).unwrap();
        std::fs::write(folder.join(&file), b"a program").unwrap();
        std::fs::write(folder.join("helpers/other"), b"another system's").unwrap();
        std::fs::write(
            folder.join("pane.json"),
            format!(
                r#"{{ "manifestVersion": 1, "title": "Tool", "apiVersion": "0.1",
                     "commands": [{{ "id": "c", "title": "C", "component": "command.wasm" }}],
                     "helpers": [{{ "id": "tool", "targets": {{ "{}": "{file}", "{other}": "helpers/other" }} }}] }}"#,
                target.id()
            ),
        )
        .unwrap();
        let staging = dir.path().join("staging");
        stage_package(&PaneManifest, &folder, &staging).unwrap();
        assert!(staging.join("pane.json").is_file());
        assert_eq!(std::fs::read(staging.join(&file)).unwrap(), b"a program");
        assert!(!staging.join("helpers/other").exists());
        // The build adds the components.
        assert!(!staging.join("command.wasm").exists());
    }
}
