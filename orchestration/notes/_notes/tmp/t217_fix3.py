import sys

ROOT = sys.argv[1]


def edit(p, pairs):
    p = ROOT + "/" + p
    s = open(p, encoding="utf-8").read()
    for a, b in pairs:
        assert s.count(a) == 1, (p, a, s.count(a))
        s = s.replace(a, b)
    open(p, "w", encoding="utf-8", newline="\n").write(s)


edit("crates/pane-core/src/local_channel.rs", [
("""    use ::windows::Win32::Foundation::{ERROR_PIPE_BUSY, HLOCAL, LocalFree};
    use ::windows::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
    };
    use ::windows::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
    use ::windows::core::PCWSTR;

    use tokio::net::windows::named_pipe::{
        ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions,
    };

    use crate::atomic::owner_only::user_sid;
""",
"""    use ::windows::Win32::Foundation::ERROR_PIPE_BUSY;

    use tokio::net::windows::named_pipe::{
        ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions,
    };

    use crate::atomic::owner_only::{user_sid, with_attributes};
"""),
("""    fn create(path: &Path, sddl: &str, first: bool) -> io::Result<NamedPipeServer> {
        let wide = crate::util::wide(sddl);
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        // SAFETY: `wide` is NUL-terminated; the descriptor is freed below.
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(wide.as_ptr()),
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )
        }
        .map_err(|error| io::Error::from_raw_os_error(error.code().0 & 0xFFFF))?;
        let mut attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0,
            bInheritHandle: false.into(),
        };
        let mut options = ServerOptions::new();
        options
            .first_pipe_instance(first)
            .reject_remote_clients(true);
        // SAFETY: `attributes` is a valid SECURITY_ATTRIBUTES, and its
        // descriptor lives until after the call.
        let created = unsafe {
            options.create_with_security_attributes_raw(path, (&raw mut attributes).cast())
        };
        // SAFETY: allocated by the conversion above with LocalAlloc.
        unsafe { LocalFree(Some(HLOCAL(descriptor.0))) };
        created
    }""",
"""    fn create(path: &Path, sddl: &str, first: bool) -> io::Result<NamedPipeServer> {
        let mut options = ServerOptions::new();
        options
            .first_pipe_instance(first)
            .reject_remote_clients(true);
        with_attributes(sddl, |attributes| {
            let attributes = std::ptr::from_ref(attributes).cast_mut().cast();
            // SAFETY: valid SECURITY_ATTRIBUTES for the call, which only
            // reads them.
            unsafe { options.create_with_security_attributes_raw(path, attributes) }
        })?
    }"""),
])

edit("crates/pane-core/src/atomic.rs", [
("""    /// Creates the folder at `path`, which must not exist yet (its parent
    /// must), with a protected DACL giving full control to this user and
    /// SYSTEM only, inherited by everything created in it: the file
    /// index's folder (#175).
    pub(crate) fn create_dir(path: &Path) -> io::Result<()> {
        use ::windows::Win32::Storage::FileSystem::CreateDirectoryW;
        let sddl = format!("D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;{})", user_sid()?);
        let sddl = crate::util::wide(&sddl);
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        // SAFETY: `sddl` is NUL-terminated; the descriptor is freed below.
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(sddl.as_ptr()),
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )
        }
        .map_err(failed)?;
        let attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0,
            bInheritHandle: false.into(),
        };
        let wide = crate::util::wide(path);
        // SAFETY: `wide` is NUL-terminated and `attributes` valid for the
        // call.
        let created = unsafe { CreateDirectoryW(PCWSTR(wide.as_ptr()), Some(&attributes)) };
        // SAFETY: allocated by the conversion above with LocalAlloc.
        unsafe { LocalFree(Some(HLOCAL(descriptor.0))) };
        created.map_err(failed)
    }

    /// Creates the file at `path`, which must not exist yet, for writing,
    /// with a protected DACL giving full control to this user and SYSTEM
    /// only.
    pub(super) fn create_new(path: &Path) -> io::Result<File> {
        let sddl = format!("D:P(A;;FA;;;SY)(A;;FA;;;{})", user_sid()?);
        let sddl = crate::util::wide(&sddl);
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        // SAFETY: `sddl` is NUL-terminated; the descriptor is freed below.
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(sddl.as_ptr()),
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )
        }
        .map_err(failed)?;
        let attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0,
            bInheritHandle: false.into(),
        };
        let wide = crate::util::wide(path);
        // SAFETY: `wide` is NUL-terminated and `attributes` valid for the
        // call; the handle is owned by the returned file.
        let created = unsafe {
            CreateFileW(
                PCWSTR(wide.as_ptr()),
                GENERIC_WRITE.0,
                FILE_SHARE_NONE,
                Some(&attributes),
                CREATE_NEW,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
        };
        // SAFETY: allocated by the conversion above with LocalAlloc.
        unsafe { LocalFree(Some(HLOCAL(descriptor.0))) };
        let handle = created.map_err(failed)?;""",
"""    /// Runs `with` on security attributes holding the security descriptor
    /// `sddl` describes, which last for the call: for a file, a folder or
    /// the local channel's pipe (#217) only this user may open.
    pub(crate) fn with_attributes<T>(
        sddl: &str,
        with: impl FnOnce(&SECURITY_ATTRIBUTES) -> T,
    ) -> io::Result<T> {
        let sddl = crate::util::wide(sddl);
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        // SAFETY: `sddl` is NUL-terminated; the descriptor is freed below.
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(sddl.as_ptr()),
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )
        }
        .map_err(failed)?;
        let attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0,
            bInheritHandle: false.into(),
        };
        let done = with(&attributes);
        // SAFETY: allocated by the conversion above with LocalAlloc.
        unsafe { LocalFree(Some(HLOCAL(descriptor.0))) };
        Ok(done)
    }

    /// Creates the folder at `path`, which must not exist yet (its parent
    /// must), with a protected DACL giving full control to this user and
    /// SYSTEM only, inherited by everything created in it: the file
    /// index's folder (#175).
    pub(crate) fn create_dir(path: &Path) -> io::Result<()> {
        use ::windows::Win32::Storage::FileSystem::CreateDirectoryW;
        let sddl = format!("D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;{})", user_sid()?);
        let wide = crate::util::wide(path);
        with_attributes(&sddl, |attributes| {
            // SAFETY: `wide` is NUL-terminated and `attributes` valid for
            // the call.
            unsafe { CreateDirectoryW(PCWSTR(wide.as_ptr()), Some(attributes)) }
        })?
        .map_err(failed)
    }

    /// Creates the file at `path`, which must not exist yet, for writing,
    /// with a protected DACL giving full control to this user and SYSTEM
    /// only.
    pub(super) fn create_new(path: &Path) -> io::Result<File> {
        let sddl = format!("D:P(A;;FA;;;SY)(A;;FA;;;{})", user_sid()?);
        let wide = crate::util::wide(path);
        let created = with_attributes(&sddl, |attributes| {
            // SAFETY: `wide` is NUL-terminated and `attributes` valid for
            // the call; the handle is owned by the returned file.
            unsafe {
                CreateFileW(
                    PCWSTR(wide.as_ptr()),
                    GENERIC_WRITE.0,
                    FILE_SHARE_NONE,
                    Some(attributes),
                    CREATE_NEW,
                    FILE_ATTRIBUTE_NORMAL,
                    None,
                )
            }
        })?;
        let handle = created.map_err(failed)?;"""),
])

# BuildOutput: named fields.
edit("crates/pane-build/src/build.rs", [
("""#[derive(Clone)]
pub(crate) struct BuildOutput(Arc<Mutex<Printed>>, Option<Echo>);""",
"""#[derive(Clone)]
pub(crate) struct BuildOutput {
    printed: Arc<Mutex<Printed>>,
    echo: Option<Echo>,
}"""),
("""        BuildOutput(
            Arc::new(Mutex::new(Printed {
                tail: VecDeque::new(),
                bytes: 0,
                dropped: 0,
                log,
            })),
            None,
        )
    }

    /// This output, each line of which `echo` also shows.
    pub(crate) fn echoing(self, echo: Option<Echo>) -> BuildOutput {
        BuildOutput(self.0, echo)
    }

    pub(crate) fn line(&self, line: &str) {
        if let Some(echo) = &self.1 {
            echo(line);
        }
        let mut printed = self.0.lock().unwrap_or_else(|p| p.into_inner());""",
"""        BuildOutput {
            printed: Arc::new(Mutex::new(Printed {
                tail: VecDeque::new(),
                bytes: 0,
                dropped: 0,
                log,
            })),
            echo: None,
        }
    }

    /// This output, each line of which `echo` also shows.
    pub(crate) fn echoing(self, echo: Option<Echo>) -> BuildOutput {
        BuildOutput { echo, ..self }
    }

    pub(crate) fn line(&self, line: &str) {
        if let Some(echo) = &self.echo {
            echo(line);
        }
        let mut printed = self.printed.lock().unwrap_or_else(|p| p.into_inner());"""),
("""    pub(crate) fn tail(&self) -> (Vec<String>, usize) {
        let printed = self.0.lock().unwrap_or_else(|p| p.into_inner());""",
"""    pub(crate) fn tail(&self) -> (Vec<String>, usize) {
        let printed = self.printed.lock().unwrap_or_else(|p| p.into_inner());"""),
])

# An alias for the callback that asks pane-ext to build.
edit("crates/pane-core/src/launcher/developing.rs", [
("""/// Who builds a developed package.
enum Driver {""",
"""/// Asks `pane-ext` to build a package it develops now, as a save would.
pub(crate) type BuildNow = Arc<dyn Fn() + Send + Sync>;

/// Who builds a developed package.
enum Driver {"""),
("""        /// Asks `pane-ext` to build the package now, as a save would.
        build: Arc<dyn Fn() + Send + Sync>,""",
"""        build: BuildNow,"""),
("""        command: &str,
        build: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<(Remote, Receiver<LogLine>), String> {""",
"""        command: &str,
        build: BuildNow,
    ) -> Result<(Remote, Receiver<LogLine>), String> {"""),
])
edit("crates/pane-core/src/launcher.rs", [
("""pub(crate) use developing::Remote;""", """pub(crate) use developing::{BuildNow, Remote};"""),
])
edit("crates/pane-core/src/local_channel.rs", [
("""use crate::launcher::{InstallPreview, Launcher, Remote};""",
"""use crate::launcher::{BuildNow, InstallPreview, Launcher, Remote};"""),
("""        let build: Arc<dyn Fn() + Send + Sync> = {""", """        let build: BuildNow = {"""),
])
