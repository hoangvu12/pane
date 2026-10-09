import sys

ROOT = sys.argv[1]


def edit(p, pairs):
    p = ROOT + "/" + p
    s = open(p, encoding="utf-8").read()
    for a, b in pairs:
        assert s.count(a) == 1, (p, a, s.count(a))
        s = s.replace(a, b)
    open(p, "w", encoding="utf-8", newline="\n").write(s)


edit("crates/pane-ext/src/dev.rs", [
("""//! It reaches the running Pane over the local channel
//! (`pane_core::local_channel`), starting Pane if none answers (see
//! `start`), and runs development mode's session, the `pane-build` crate's
//! as Pane's own development mode does: the builds run here and print here,
//! the compiler's errors included. The first build that succeeds is handed
//! to Pane; if Pane has not installed the folder, it shows its install
//! preview first, which the author confirms in Pane. After that each save
//! builds the package here again, and Pane reloads each build that
//! succeeds, while a build that fails leaves it running the working code.""",
"""//! It runs development mode's session, the `pane-build` crate's as Pane's
//! own development mode does, so the builds run here and print here, the
//! compiler's errors included. Meanwhile it reaches the running Pane over
//! the local channel (`pane_core::local_channel`), starting Pane if none
//! answers (see `start`), and stops if there is none to start. The first
//! build that succeeds is handed to Pane; if Pane has not installed the
//! folder, it shows its install preview first, which the author confirms in
//! Pane. After that each save builds the package here again, and Pane
//! reloads each build that succeeds, while a build that fails leaves it
//! running the working code."""),
("""    let endpoint = Endpoint::from_env()
        .map_err(|error| format!("Pane's endpoint for this user is not known: {error}"))?;
    let (sender, events) = crate::start::reach(&endpoint)?;
    // Asked before the first build is handed over, so that none of Pane's
    // messages about it are missed.
    sender.send(&Request::Subscribe);
    let command = prepared.command();""",
"""    let endpoint = Endpoint::from_env()
        .map_err(|error| format!("Pane's endpoint for this user is not known: {error}"))?;
    let command = prepared.command();"""),
("""    let (answers, answered) = mpsc::channel();
    {
        let shared = shared.clone();
        let session = session.clone();
        std::thread::Builder::new()
            .name("pane-ext-events".into())
            .spawn(move || read_events(events, answers, &shared, &session))
            .map_err(|error| format!("a thread could not start: {error}"))?;
    }
    worker.start(Handing {
        folder,
        command,
        sender,
        answers: answered,
        developing: false,
        shared,
    });
    session.build_now();""",
"""    // Pane is reached, or started, while the first build runs.
    let (found, reached) = mpsc::channel();
    {
        let shared = shared.clone();
        let session = session.clone();
        std::thread::Builder::new()
            .name("pane-ext-events".into())
            .spawn(move || follow_pane(&endpoint, found, &shared, &session))
            .map_err(|error| format!("a thread could not start: {error}"))?;
    }
    worker.start(Handing {
        folder,
        command,
        pane: None,
        reached,
        developing: false,
        shared,
    });
    session.build_now();"""),
("""/// The session's host: hands each build that succeeds to the running Pane.
struct Handing {
    folder: PathBuf,
    command: String,
    sender: Sender,
    /// Pane's answers to `develop`; its other events are printed as they
    /// come.
    answers: Receiver<Event>,
    /// Whether Pane develops the package yet: from then on, Pane tells of
    /// the builds in the package's log.
    developing: bool,
    shared: Arc<Shared>,
}""",
"""/// The running Pane, once reached.
struct Reached {
    sender: Sender,
    /// Pane's answers to `develop`; its other events are printed as they
    /// come.
    answers: Receiver<Event>,
}

/// The session's host: hands each build that succeeds to the running Pane.
struct Handing {
    folder: PathBuf,
    command: String,
    /// Pane, once reached; it comes on `reached`.
    pane: Option<Reached>,
    reached: Receiver<Reached>,
    /// Whether Pane develops the package yet: from then on, Pane tells of
    /// the builds in the package's log.
    developing: bool,
    shared: Arc<Shared>,
}"""),
("""    fn building(&self, command: &str) {
        if self.developing {
            self.sender.send(&Request::Building);
        } else {
            println!("pane-ext: building with `{command}`");
        }
    }

    fn failed(&self, failure: &BuildFailure, _command: &str) {
        if self.developing {
            // Pane keeps running the working code.
            self.sender.send(&Request::Failed {""",
"""    fn building(&self, command: &str) {
        match &self.pane {
            Some(pane) if self.developing => {
                pane.sender.send(&Request::Building);
            }
            _ => println!("pane-ext: building with `{command}`"),
        }
    }

    fn failed(&self, failure: &BuildFailure, _command: &str) {
        if let Some(pane) = &self.pane
            && self.developing
        {
            // Pane keeps running the working code.
            pane.sender.send(&Request::Failed {"""),
("""    fn deliver(&mut self, _claim: &mut (), staging: &Path) -> bool {
        let develop = Request::Develop {
            folder: self.folder.clone(),
            staging: staging.to_path_buf(),
            command: self.command.clone(),
        };
        if !self.sender.send(&develop) {
            return false;
        }
        loop {
            match self.answers.recv() {""",
"""    fn deliver(&mut self, _claim: &mut (), staging: &Path) -> bool {
        if self.pane.is_none() {
            // Never sent when no Pane was found, which ends the development.
            let Ok(pane) = self.reached.recv() else {
                return false;
            };
            self.pane = Some(pane);
        }
        let Some(pane) = &self.pane else {
            return false;
        };
        let develop = Request::Develop {
            folder: self.folder.clone(),
            staging: staging.to_path_buf(),
            command: self.command.clone(),
        };
        if !pane.sender.send(&develop) {
            return false;
        }
        loop {
            match pane.answers.recv() {"""),
("""/// Prints Pane's messages and the package's log as they come, has the
/// session build when Pane asks, and passes Pane's answers to `develop` to
/// the session's thread, until the development ends.
fn read_events(""",
"""/// Reaches the Pane listening on `endpoint`, starting one if none does, and
/// hands it to the session's thread through `found`; then reads its events
/// (see [`read_events`]). Ends the development if no Pane can be reached.
fn follow_pane(
    endpoint: &Endpoint,
    found: mpsc::Sender<Reached>,
    shared: &Shared,
    session: &Session,
) {
    let (sender, events) = match crate::start::reach(endpoint) {
        Ok(reached) => reached,
        Err(message) => return shared.end(Ending::Failed(message)),
    };
    // Asked before the first build is handed over, so that none of Pane's
    // messages about it are missed.
    sender.send(&Request::Subscribe);
    let (answers, answered) = mpsc::channel();
    let reached = Reached {
        sender,
        answers: answered,
    };
    if found.send(reached).is_ok() {
        read_events(events, answers, shared, session);
    }
}

/// Prints Pane's messages and the package's log as they come, has the
/// session build when Pane asks, and passes Pane's answers to `develop` to
/// the session's thread, until the development ends.
fn read_events("""),
])

edit("crates/pane-ext/tests/dev.rs", [
("""//! development. With no Pane listening and none to start, `pane-ext dev`
//! says where it looked for one, before it builds anything.""",
"""//! development. With no Pane listening and none to start, `pane-ext dev`
//! says where it looked for one, without waiting for its build."""),
("""//! needs). `PANE_APP` names no program, so that `pane-ext` never starts a
//! Pane of its own here.""",
"""//! needs). `PANE_APP` names a file that does not exist, so that `pane-ext`
//! never starts a Pane of its own here."""),
("""    // It looks before it builds anything.
""", """    // It looks while the first build runs, and stops it.
"""),
])

# 2. Refuse a develop after Pane ended the development.
edit("crates/pane-core/src/local_channel.rs", [
("""        // Ended in Pane meanwhile (stopped there, or taken over by another
        // pane-ext): it is developed again.
        if self
            .developed
            .as_ref()
            .is_some_and(|remote| !remote.is_current())
        {
            self.developed = None;
            self.following = false;
        }
        if let Some(remote) = &self.developed {""",
"""        if let Some(remote) = &self.developed {
            // Ended in Pane meanwhile (stopped there, or taken over by
            // another pane-ext): it is not taken back.
            if !remote.is_current() {
                let message = format!("Pane no longer develops {}", remote.title());
                return self.refuse(message);
            }"""),
("""            if let Some(folder) = path.parent() {
                own_folder(folder)?;
            }""",
"""            if let Some(folder) = path.parent().filter(|folder| !folder.as_os_str().is_empty()) {
                own_folder(folder)?;
            }"""),
("""    pub(super) async fn connect(path: &Path) -> io::Result<UnixStream> {
        UnixStream::connect(path).await
    }""",
"""    /// A connection to the socket at `path`, if its folder is this user's
    /// alone: a folder another user made in the temporary folder first is
    /// not trusted with the builds.
    pub(super) async fn connect(path: &Path) -> io::Result<UnixStream> {
        if let Some(folder) = path.parent().filter(|folder| !folder.as_os_str().is_empty())
            && let Ok(metadata) = std::fs::symlink_metadata(folder)
            && !is_own(&metadata)
        {
            return Err(not_own(folder));
        }
        UnixStream::connect(path).await
    }"""),
("""        let metadata = std::fs::symlink_metadata(folder)?;
        if !metadata.is_dir() || metadata.uid() != uid() || metadata.mode() & 0o077 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "{} is not a folder only this user can open",
                    folder.display()
                ),
            ));
        }
        Ok(())
    }""",
"""        let metadata = std::fs::symlink_metadata(folder)?;
        if !is_own(&metadata) {
            return Err(not_own(folder));
        }
        Ok(())
    }

    /// Whether `metadata` is of a folder only this user can open.
    fn is_own(metadata: &std::fs::Metadata) -> bool {
        metadata.is_dir() && metadata.uid() == uid() && metadata.mode() & 0o077 == 0
    }

    fn not_own(folder: &Path) -> io::Error {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "{} is not a folder only this user can open",
                folder.display()
            ),
        )
    }"""),
])

# 3. Pane honours PANE_CHANNEL in every build, as pane-ext does.
edit("crates/pane/src/main.rs", [
("""        // of a folder Pane has not installed. A development build listens
        // where PANE_CHANNEL says, if it says, as the tests and a second Pane
        // beside an installed one need; a release build only on the user's
        // own endpoint.
        #[cfg(debug_assertions)]
        let endpoint = local_channel::Endpoint::from_env();
        #[cfg(not(debug_assertions))]
        let endpoint = local_channel::Endpoint::for_this_user();
        match endpoint.and_then(|endpoint| local_channel::serve(developing, &endpoint)) {""",
"""        // of a folder Pane has not installed. It listens where PANE_CHANNEL
        // says, if it says, as pane-ext looks there: a second Pane beside an
        // installed one, and the tests.
        let endpoint = local_channel::Endpoint::from_env();
        match endpoint.and_then(|endpoint| local_channel::serve(developing, &endpoint)) {"""),
])

edit("docs/development-mode.md", [
("""   connections from other users unanswered. `PANE_CHANNEL` names another
   endpoint (to `pane-ext` always; to Pane only in a development build).""",
"""   connections from other users unanswered; `pane-ext` does not connect
   through such a folder that is not the user's own. `PANE_CHANNEL` names
   another endpoint, to both."""),
("""2. If no Pane answers, it starts one: the program `PANE_APP` names, else""",
"""2. While its first build runs (step 3), if no Pane answers, it starts one:
   the program `PANE_APP` names, else"""),
("""   and waits up to a minute for it to listen. Found nowhere, it says where
   it looked and exits, before building anything.""",
"""   and waits up to a minute for it to listen. Found nowhere, it says where
   it looked and exits, stopping the build."""),
("""  that build; that a second `pane-ext dev` of the same package takes the
  development over; and that `pane-ext` reaches Pane before its first
  build, so that a missing Pane is reported at once.""",
"""  that build; that a second `pane-ext dev` of the same package takes the
  development over, and the first is then refused rather than taking it
  back; and that `pane-ext` reaches or starts Pane while its first build
  runs, so that a missing Pane is reported at once."""),
("""  the development stopped when `pane-ext` is killed. With no Pane listening
  and none to start, it says where it looked, before building. Its unit""",
"""  the development stopped when `pane-ext` is killed. With no Pane listening
  and none to start, it says where it looked. Its unit"""),
("""- That another user cannot open the endpoint is checked in CI through its
  permissions (the socket's and its folder's modes, the pipe's DACL), not
  by connecting as another user.""",
"""- That another user cannot open the endpoint is checked in CI through its
  permissions (the socket's and its folder's modes, the pipe's DACL), not
  by connecting as another user. On Windows, `pane-ext` does not check who
  created the pipe it connects to: another user's pipe of that name,
  created while Pane is not running, would receive its requests (the
  folder and the staged build's path)."""),
])

edit("CONTEXT.md", [
("""An installed local package whose source folder Pane watches while its author works on it: each save runs the package's documented build command in that folder, staging the components under Pane's data folder, and a build that succeeds reloads the package from there, while one that fails keeps its working code and shows the build's diagnostics.""",
"""An installed local package whose source folder Pane watches while its author works on it: each save runs the package's documented build command in that folder, staging the components under Pane's data folder, and a build that succeeds reloads the package from there, while one that fails keeps its working code and shows the build's diagnostics. Under `pane-ext dev` the watching and the builds are `pane-ext`'s, in the author's terminal, and Pane reloads each build it hands over the local channel."""),
])
