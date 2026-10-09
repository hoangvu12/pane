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
("""            if let Some(folder) = path.parent().filter(|folder| !folder.as_os_str().is_empty()) {
                own_folder(folder)?;
            }""",
"""            if let Some(folder) = folder_of(path) {
                own_folder(folder)?;
            }"""),
("""        if let Some(folder) = path.parent().filter(|folder| !folder.as_os_str().is_empty())
            && let Ok(metadata)""",
"""        if let Some(folder) = folder_of(path)
            && let Ok(metadata)"""),
("""    /// Whether `metadata` is of a folder only this user can open.""",
"""    /// The folder the socket at `path` is in, if it names one.
    fn folder_of(path: &Path) -> Option<&Path> {
        let folder = path.parent()?;
        (!folder.as_os_str().is_empty()).then_some(folder)
    }

    /// Whether `metadata` is of a folder only this user can open."""),
])
