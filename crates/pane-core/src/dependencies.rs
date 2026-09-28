//! Dependencies between extension packages: what installing a package
//! means for the packages it declares under `dependencies` in its
//! `pane.json`, worked out before anything is installed, and installing
//! them together.
//!
//! - A **required** dependency that is not installed is installed with the
//!   package, before it, after its own required dependencies. One that is
//!   installed is used as it is: Pane never replaces an installed copy while
//!   installing another package, so an installed copy acts as pinned, and
//!   one the user disabled stays disabled.
//! - An **optional** dependency is never installed with the package; it is
//!   listed, and used when the user installs it.
//! - A dependency is **compatible** when its package supports this system
//!   and publishes every operation the declaring package names, at the
//!   version it names. There is one copy per source, so two packages naming
//!   different versions of one operation of the same source conflict: Pane
//!   explains it rather than solving for several versions.
//! - **Cycles** are allowed: each source is visited once, so packages that
//!   require each other are installed together. At most
//!   [`MAX_INSTALLED_WITH`] packages are installed with the requested one.
//! - Anything that stops a required dependency from being installed stops
//!   the whole install before anything changes. A failure while installing
//!   undoes what this install added, so a failed install never leaves a
//!   package installed without its required dependencies; what could not
//!   be undone is reported as installed.

use std::path::Path;

use crate::packages::{
    InstalledPackage, Manifest, ManifestDependency, PackageError, PackageIdentity, SourcePackage,
    Store,
};
use crate::platform::{self, Platform};

/// The most packages Pane installs along with the requested one.
pub const MAX_INSTALLED_WITH: usize = 16;

/// What installing a package means for its dependencies.
pub(crate) struct Plan {
    /// The required packages to install before the requested one, each after
    /// its own required dependencies.
    pub install: Vec<SourcePackage>,
    /// The required dependencies, as the preview lists them, first found
    /// first.
    pub required: Vec<Required>,
    /// The requested package's optional dependencies and those it does not
    /// need on this system, as the preview lists them.
    pub others: Vec<String>,
    /// Why the package cannot be installed; empty when it can.
    pub problems: Vec<String>,
}

/// A required dependency the plan found.
pub(crate) struct Required {
    pub title: String,
    pub identity: PackageIdentity,
    /// The title of the package requiring it, when that is not the
    /// requested package.
    pub for_package: Option<String>,
    pub state: RequiredState,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum RequiredState {
    /// It is not installed, and is installed with the package.
    Install,
    /// It is installed and enabled.
    Installed,
    /// It is installed and the user disabled it; it stays disabled.
    Disabled,
}

impl Plan {
    /// The preview's lines about the dependencies.
    pub fn lines(&self, title: &str) -> Vec<String> {
        let mut lines: Vec<String> = self
            .required
            .iter()
            .map(|required| {
                let needs = match &required.for_package {
                    Some(package) => format!("Requires (for {package})"),
                    None => "Requires".to_owned(),
                };
                let name = &required.title;
                match required.state {
                    RequiredState::Install => {
                        format!(
                            "{needs}: {name}, installed with it from {}",
                            required.identity
                        )
                    }
                    RequiredState::Installed => format!("{needs}: {name}, already installed"),
                    RequiredState::Disabled => format!(
                        "{needs}: {name}, which you disabled: it stays disabled, and {title} \
                         cannot use it until you enable it in Manage extensions"
                    ),
                }
            })
            .collect();
        lines.extend(self.others.iter().cloned());
        lines
    }

    /// The titles of the packages installed with the requested one.
    pub fn installed_with(&self) -> Vec<String> {
        self.install
            .iter()
            .map(|package| package.manifest.title.clone())
            .collect()
    }
}

/// Works out what installing `requested` means for its dependencies, given
/// the `installed` packages and `read`, which reads and validates the
/// package in a folder. Changes nothing.
pub(crate) fn plan(
    requested: &SourcePackage,
    installed: &[InstalledPackage],
    read: impl FnMut(&Path) -> Result<SourcePackage, PackageError>,
) -> Plan {
    let mut planner = Planner {
        installed,
        read,
        requested: requested.identity.clone(),
        seen: vec![requested.identity.clone()],
        demands: Vec::new(),
        plan: Plan {
            install: Vec::new(),
            required: Vec::new(),
            others: Vec::new(),
            problems: Vec::new(),
        },
        too_many: false,
    };
    planner.visit(requested);
    planner.check_demands(requested);
    planner.plan
}

/// One package's need of an operation of another.
struct Demand {
    requirer: String,
    target: PackageIdentity,
    dependency: ManifestDependency,
}

struct Planner<'a, R> {
    installed: &'a [InstalledPackage],
    read: R,
    requested: PackageIdentity,
    /// The requested package and every required dependency visited.
    seen: Vec<PackageIdentity>,
    demands: Vec<Demand>,
    plan: Plan,
    too_many: bool,
}

impl<R: FnMut(&Path) -> Result<SourcePackage, PackageError>> Planner<'_, R> {
    fn visit(&mut self, package: &SourcePackage) {
        let title = &package.manifest.title;
        let is_requested = package.identity == self.requested;
        for dependency in &package.manifest.dependencies {
            let id = &dependency.id;
            if !dependency.needed_here() {
                if is_requested {
                    self.plan.others.push(format!(
                        "Not needed on this system: `{id}` from {} (only on {})",
                        dependency.source,
                        systems(dependency.platforms.as_deref().unwrap_or_default())
                    ));
                }
                continue;
            }
            let target = package.identity.dependency(&dependency.source);
            if !dependency.required {
                if is_requested {
                    let line = self.optional_line(dependency, target);
                    self.plan.others.push(line);
                }
                continue;
            }
            let target = match target {
                Ok(target) => target,
                Err(reason) => {
                    self.plan.problems.push(format!(
                        "{title} requires `{id}` from {}: {reason}",
                        dependency.source
                    ));
                    continue;
                }
            };
            if target == package.identity {
                self.plan
                    .problems
                    .push(format!("{title} names itself as its dependency `{id}`"));
                continue;
            }
            self.demands.push(Demand {
                requirer: title.clone(),
                target: target.clone(),
                dependency: dependency.clone(),
            });
            if self.seen.contains(&target) {
                continue;
            }
            self.seen.push(target.clone());
            let for_package = (!is_requested).then(|| title.clone());
            if let Some(installed) = self.installed.iter().find(|p| p.identity == target) {
                let state = if installed.enabled {
                    RequiredState::Installed
                } else {
                    RequiredState::Disabled
                };
                self.plan.required.push(Required {
                    title: installed.title(),
                    identity: target,
                    for_package,
                    state,
                });
                continue;
            }
            if self.pending() >= MAX_INSTALLED_WITH {
                if !self.too_many {
                    self.too_many = true;
                    self.plan.problems.push(format!(
                        "Installing it would install more than {MAX_INSTALLED_WITH} other \
                         extensions with it; Pane installs at most {MAX_INSTALLED_WITH} at once"
                    ));
                }
                continue;
            }
            let folder = target
                .local_folder()
                .expect("a local dependency has a folder")
                .to_path_buf();
            match (self.read)(&folder) {
                Ok(source) => {
                    self.plan.required.push(Required {
                        title: source.manifest.title.clone(),
                        identity: target,
                        for_package,
                        state: RequiredState::Install,
                    });
                    self.visit(&source);
                    self.plan.install.push(source);
                }
                Err(error) => self.plan.problems.push(format!(
                    "{title} requires `{id}` from {}, which cannot be installed: {error}",
                    folder.display()
                )),
            }
        }
    }

    /// How many packages the plan installs so far, those still being
    /// visited included.
    fn pending(&self) -> usize {
        self.plan
            .required
            .iter()
            .filter(|required| required.state == RequiredState::Install)
            .count()
    }

    fn optional_line(
        &self,
        dependency: &ManifestDependency,
        target: Result<PackageIdentity, String>,
    ) -> String {
        let id = &dependency.id;
        let target = match target {
            Ok(target) => target,
            Err(reason) => {
                return format!(
                    "Optional: `{id}` from {}, which Pane cannot install: {reason}",
                    dependency.source
                );
            }
        };
        match self.installed.iter().find(|p| p.identity == target) {
            Some(installed) if !installed.enabled => format!(
                "Optional: {}, installed but disabled: it stays disabled",
                installed.title()
            ),
            Some(installed) => format!("Optional: {}, installed", installed.title()),
            None => format!(
                "Optional: `{id}` from {target}, not installed: Pane does not install it; \
                 install it yourself to use it"
            ),
        }
    }

    /// Checks that every required dependency found publishes what its
    /// requirers call, with one version of each operation.
    fn check_demands(&mut self, requested: &SourcePackage) {
        let mut targets: Vec<&PackageIdentity> = Vec::new();
        for demand in &self.demands {
            if !targets.contains(&&demand.target) {
                targets.push(&demand.target);
            }
        }
        let mut problems = Vec::new();
        for target in targets {
            let demands: Vec<&Demand> = self
                .demands
                .iter()
                .filter(|demand| demand.target == *target)
                .collect();
            let installed = self.installed.iter().find(|p| p.identity == *target);
            let (title, manifest) = if *target == requested.identity {
                (requested.manifest.title.clone(), Ok(&requested.manifest))
            } else if let Some(source) = self.plan.install.iter().find(|p| p.identity == *target) {
                (source.manifest.title.clone(), Ok(&source.manifest))
            } else if let Some(installed) = installed {
                (installed.title(), installed.manifest.as_ref())
            } else {
                // It could not be read, which is explained already.
                continue;
            };
            let kept = if installed.is_some() && *target != requested.identity {
                format!(
                    "; Pane does not replace the installed copy of {title} while installing \
                     another extension: update it from its folder first"
                )
            } else {
                String::new()
            };
            let manifest = match manifest {
                Ok(manifest) => manifest,
                Err(error) => {
                    for demand in demands {
                        problems.push(format!(
                            "{} requires {title}, whose installed copy cannot load: {error}",
                            demand.requirer
                        ));
                    }
                    continue;
                }
            };
            if installed.is_some()
                && let Some(reason) =
                    platform::unavailable(manifest.platforms.as_deref(), "this package")
            {
                problems.push(format!(
                    "{} requires {title}: {reason}",
                    demands[0].requirer
                ));
                continue;
            }
            for problem in unmet(&title, manifest, &demands) {
                problems.push(format!("{problem}{kept}"));
            }
        }
        self.plan.problems.extend(problems);
    }
}

/// Why `manifest`, titled `title`, does not serve `demands`: operations it
/// does not publish, at another version or not on this system, and two
/// requirers naming different versions of one operation.
fn unmet(title: &str, manifest: &Manifest, demands: &[&Demand]) -> Vec<String> {
    let mut problems = Vec::new();
    let mut checked: Vec<&str> = Vec::new();
    for demand in demands {
        for wanted in &demand.dependency.operations {
            if checked.contains(&wanted.id.as_str()) {
                continue;
            }
            checked.push(&wanted.id);
            let others: Vec<(&str, u32)> = demands
                .iter()
                .flat_map(|other| {
                    other
                        .dependency
                        .operations
                        .iter()
                        .filter(|operation| operation.id == wanted.id)
                        .map(|operation| (other.requirer.as_str(), operation.version))
                })
                .collect();
            if let Some((other, version)) = others.iter().find(|(_, v)| *v != wanted.version) {
                problems.push(format!(
                    "{} and {other} need different versions of `{}` from {title} ({} and \
                     {version}); Pane installs one copy of each source, so they conflict",
                    demand.requirer, wanted.id, wanted.version
                ));
                continue;
            }
            let requirer = &demand.requirer;
            let operation = &wanted.id;
            match manifest.operations.iter().find(|o| o.id == wanted.id) {
                None => problems.push(format!(
                    "{requirer} requires {title} to publish `{operation}`, which it does not \
                     publish"
                )),
                Some(published) if published.version != wanted.version => problems.push(format!(
                    "{requirer} requires `{operation}` version {} from {title}, which \
                         publishes version {}",
                    wanted.version, published.version
                )),
                Some(published) => {
                    if let Some(reason) =
                        platform::unavailable(published.platforms.as_deref(), "this operation")
                    {
                        problems.push(format!(
                            "{requirer} requires `{operation}` from {title}: {reason}"
                        ));
                    }
                }
            }
        }
    }
    problems
}

/// "Windows and Linux".
fn systems(platforms: &[Platform]) -> String {
    let names: Vec<String> = platforms.iter().map(Platform::to_string).collect();
    platform::join(&names)
}

/// Installing a package with its planned dependencies failed.
pub(crate) struct Failure {
    /// Why, naming the package that could not be installed.
    pub error: String,
    /// Packages this install added that could not be removed again, each
    /// with why: they stay installed.
    pub kept: Vec<(InstalledPackage, String)>,
}

/// Installs `dependencies` in order, then the requested package with
/// `last`. If any fails, removes again those this call installed, most
/// recent first, restoring the record of data Pane kept for them, so that
/// the store is as before; returns the installed packages, dependencies
/// first, or the failure.
pub(crate) fn install_all(
    store: &mut Store,
    dependencies: &[SourcePackage],
    last: impl FnOnce(&mut Store) -> Result<InstalledPackage, PackageError>,
) -> Result<(Vec<InstalledPackage>, InstalledPackage), Failure> {
    let mut added: Vec<(InstalledPackage, Option<String>)> = Vec::new();
    for dependency in dependencies {
        let retained = store
            .retained()
            .into_iter()
            .find(|retained| retained.identity == dependency.identity)
            .map(|retained| retained.title);
        match store.install(dependency) {
            Ok(installed) => added.push((installed, retained)),
            Err(error) => {
                let error = format!(
                    "Could not install {}, which it requires: {error}",
                    dependency.manifest.title
                );
                return Err(undo(store, added, error));
            }
        }
    }
    match last(store) {
        Ok(installed) => Ok((added.into_iter().map(|(p, _)| p).collect(), installed)),
        Err(error) => Err(undo(store, added, error.to_string())),
    }
}

fn undo(
    store: &mut Store,
    added: Vec<(InstalledPackage, Option<String>)>,
    error: String,
) -> Failure {
    let mut kept = Vec::new();
    for (package, retained) in added.into_iter().rev() {
        if let Err(problem) = store.uninstall(&package.identity, retained) {
            kept.push((package, problem.to_string()));
        }
    }
    Failure { error, kept }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::*;

    /// A package folder `name` in `sources`, with a component file that is
    /// never run: the store copies files, it does not check them.
    fn package(sources: &Path, name: &str) -> SourcePackage {
        let folder = sources.join(name);
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("c.wasm"), b"not run").unwrap();
        fs::write(
            folder.join("pane.json"),
            format!(
                r#"{{ "manifestVersion": 1, "title": "Package {name}", "apiVersion": "0.1",
                     "operations": [{{ "id": "echo", "version": 1, "component": "c.wasm" }}] }}"#
            ),
        )
        .unwrap();
        SourcePackage::read(&folder).unwrap()
    }

    fn titles(store: &Store) -> Vec<String> {
        store
            .installed()
            .iter()
            .map(InstalledPackage::title)
            .collect()
    }

    fn setup() -> (tempfile::TempDir, PathBuf, Store) {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        let store = Store::open(data.clone());
        (dir, data, store)
    }

    #[test]
    fn a_failure_of_the_requested_package_removes_its_dependencies_again() {
        let (dir, data, mut store) = setup();
        let b = package(dir.path(), "b");
        let c = package(dir.path(), "c");
        // Pane kept b's data from an earlier installation.
        store.retain(&b.identity, "Old b".into()).unwrap();

        let failure = install_all(&mut store, &[b.clone(), c], |_| {
            Err(PackageError::Storage("the disk is full".into()))
        })
        .expect_err("the install fails");

        assert_eq!(
            failure.error,
            "Could not update Pane's installed extensions: the disk is full"
        );
        assert!(failure.kept.is_empty());
        assert!(titles(&store).is_empty());
        // b's kept data is on record again, as before.
        let retained = store.retained();
        assert_eq!(retained.len(), 1);
        assert_eq!(retained[0].identity, b.identity);
        assert_eq!(retained[0].title, "Old b");
        // So it is after a restart, and the managed copies are gone.
        let reopened = Store::open(data.clone());
        assert!(titles(&reopened).is_empty());
        assert_eq!(reopened.retained().len(), 1);
        assert_eq!(fs::read_dir(data.join("packages")).unwrap().count(), 0);
    }

    #[test]
    fn a_failure_of_a_dependency_removes_those_installed_before_it() {
        let (dir, _data, mut store) = setup();
        let b = package(dir.path(), "b");
        let c = package(dir.path(), "c");
        // c got installed meanwhile, so installing it again fails.
        store.install(&c).unwrap();

        let failure =
            install_all(&mut store, &[b, c], |_| unreachable!()).expect_err("the install fails");

        assert!(
            failure
                .error
                .starts_with("Could not install Package c, which it requires: Already installed"),
            "{}",
            failure.error
        );
        assert_eq!(titles(&store), ["Package c"]);
    }

    #[test]
    fn a_success_installs_dependencies_first() {
        let (dir, _data, mut store) = setup();
        let a = package(dir.path(), "a");
        let b = package(dir.path(), "b");

        let (dependencies, installed) = install_all(&mut store, &[b], |store| store.install(&a))
            .ok()
            .unwrap();

        assert_eq!(dependencies.len(), 1);
        assert_eq!(installed.title(), "Package a");
        assert_eq!(titles(&store), ["Package b", "Package a"]);
    }
}
