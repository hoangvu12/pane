//! What kind of volume holds a folder (#126 "Scope and rules", #184):
//! network shares and removable drives are left out of the index unless the
//! user includes other volumes (`ScopeRules::include_other_volumes`), and a
//! network share that is included is never watched, only reconciled. The
//! scope asks through [`VolumeKinds`], so that tests can say what a folder
//! is on; the system's answer is [`volume_kind`]:
//!
//! - Windows: a network path (`\\server\share`) is a network share by its
//!   text, before any call; any other path is resolved to the root of the
//!   volume holding it (`GetVolumePathNameW`: a drive letter, a mapped
//!   drive's letter, or the folder a volume is mounted in) and that root's
//!   drive type asked (`GetDriveTypeW`): remote is a network share,
//!   removable or CD-ROM a removable drive.
//! - macOS: `statfs`'s flags: a file system without `MNT_LOCAL` is a network
//!   share, one with `MNT_REMOVABLE` (removable media) a removable drive.
//! - Linux: `statfs`'s file system type: NFS, SMB, CIFS, FUSE (sshfs, rclone
//!   and the like), AFS and Lustre are network shares, FAT and exFAT
//!   removable drives.
//!
//! A folder the system says nothing about (it is missing, the call failed)
//! is taken as local, so that a root that is away keeps its entries.

use std::path::Path;
use std::sync::Arc;

/// What kind of volume holds a folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VolumeKind {
    /// A disk of this computer, or one the system does not tell apart.
    Local,
    /// A network share: an SMB or NFS share, a mapped drive, a FUSE mount.
    Network,
    /// A removable drive: a USB stick, a memory card, a disc.
    Removable,
}

/// Tells what kind of volume holds a folder (see
/// [`super::Scope::with_volumes`]): the system's ([`volume_kind`]) unless
/// a test says otherwise.
pub type VolumeKinds = Arc<dyn Fn(&Path) -> VolumeKind + Send + Sync>;

/// The kind of volume holding `path`, as the system says (see the module
/// docs); [`VolumeKind::Local`] when it does not say.
pub fn volume_kind(path: &Path) -> VolumeKind {
    system_kind(path)
}

/// The kind a Windows drive type (`GetDriveTypeW`'s answer) stands for.
#[cfg(any(windows, test))]
fn drive_kind(drive_type: u32) -> VolumeKind {
    // The values of `DRIVE_REMOVABLE`, `DRIVE_REMOTE` and `DRIVE_CDROM`.
    match drive_type {
        4 => VolumeKind::Network,
        2 | 5 => VolumeKind::Removable,
        _ => VolumeKind::Local,
    }
}

/// The kind macOS's `statfs` flags (`f_flags`) stand for.
#[cfg(any(target_os = "macos", test))]
fn mount_kind(flags: u32) -> VolumeKind {
    /// `MNT_LOCAL` in `<sys/mount.h>`: a file system of this computer.
    const MNT_LOCAL: u32 = 0x0000_1000;
    /// `MNT_REMOVABLE` in `<sys/mount.h>` (not in the `libc` crate):
    /// mounted from removable media.
    const MNT_REMOVABLE: u32 = 0x0000_0200;
    if flags & MNT_LOCAL == 0 {
        VolumeKind::Network
    } else if flags & MNT_REMOVABLE != 0 {
        VolumeKind::Removable
    } else {
        VolumeKind::Local
    }
}

/// The kind Linux's `statfs` file system type (`f_type`) stands for.
#[cfg(any(target_os = "linux", test))]
fn file_system_kind(kind: i64) -> VolumeKind {
    const NETWORK: [i64; 7] = [
        0x6969,      // NFS
        0x517B,      // SMB
        0xFF53_4D42, // CIFS
        0xFE53_4D42, // SMB2
        0x6573_5546, // FUSE (sshfs, rclone and the like)
        0x5346_414F, // AFS
        0x0BD0_0BD0, // Lustre
    ];
    const REMOVABLE: [i64; 2] = [
        0x0000_4D44, // FAT
        0x2011_BAB0, // exFAT
    ];
    if NETWORK.contains(&kind) {
        VolumeKind::Network
    } else if REMOVABLE.contains(&kind) {
        VolumeKind::Removable
    } else {
        VolumeKind::Local
    }
}

#[cfg(windows)]
fn system_kind(path: &Path) -> VolumeKind {
    use windows::Win32::Storage::FileSystem::GetDriveTypeW;
    use windows::core::PCWSTR;

    if crate::files::is_network_path(path) {
        return VolumeKind::Network;
    }
    let Some(root) = volume_root(path) else {
        return VolumeKind::Local;
    };
    // SAFETY: a NUL-terminated root folder.
    drive_kind(unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) })
}

/// The root folder of the volume holding `path`, NUL-terminated: `C:\`, a
/// mapped drive's `Z:\`, or `C:\Mounts\Stick\` for a volume mounted in a
/// folder; a drive letter's root when the system does not resolve it (a
/// mapped drive that is not connected now).
#[cfg(windows)]
fn volume_root(path: &Path) -> Option<Vec<u16>> {
    use std::path::{Component, Prefix};

    use windows::Win32::Storage::FileSystem::GetVolumePathNameW;
    use windows::core::PCWSTR;

    let mut buffer = [0u16; 1024];
    let wide = crate::util::wide(path);
    // SAFETY: a NUL-terminated path and a writable buffer.
    if unsafe { GetVolumePathNameW(PCWSTR(wide.as_ptr()), &mut buffer) }.is_ok() {
        let len = buffer.iter().position(|&unit| unit == 0)?;
        return Some(buffer[..=len].to_vec());
    }
    match path.components().next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => {
                Some(crate::util::wide(format!("{}:\\", char::from(letter))))
            }
            _ => None,
        },
        _ => None,
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn system_kind(path: &Path) -> VolumeKind {
    use std::os::unix::ffi::OsStrExt;

    let Ok(path) = std::ffi::CString::new(path.as_os_str().as_bytes()) else {
        return VolumeKind::Local;
    };
    // SAFETY: a plain C structure, for which all zeroes is a valid value.
    let mut stats: libc::statfs = unsafe { std::mem::zeroed() };
    // SAFETY: a NUL-terminated path and a structure of the call's own type.
    if unsafe { libc::statfs(path.as_ptr(), &mut stats) } != 0 {
        return VolumeKind::Local;
    }
    #[cfg(target_os = "macos")]
    {
        mount_kind(stats.f_flags)
    }
    #[cfg(target_os = "linux")]
    {
        // `f_type` is an `i64` on the 64-bit targets Pane builds for, and
        // narrower elsewhere.
        #[allow(clippy::unnecessary_cast)]
        let kind = stats.f_type as i64;
        file_system_kind(kind)
    }
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
fn system_kind(_path: &Path) -> VolumeKind {
    VolumeKind::Local
}

#[cfg(test)]
pub(crate) mod tests {
    use std::path::PathBuf;

    use super::*;

    /// What a test says a folder is on, standing for the system's answer: a
    /// network share under `share`, a removable drive under `stick`, a
    /// local disk elsewhere.
    pub(crate) fn fake_volumes(share: &Path, stick: &Path) -> VolumeKinds {
        let share: PathBuf = share.to_path_buf();
        let stick: PathBuf = stick.to_path_buf();
        Arc::new(move |path: &Path| {
            if path.starts_with(&share) {
                VolumeKind::Network
            } else if path.starts_with(&stick) {
                VolumeKind::Removable
            } else {
                VolumeKind::Local
            }
        })
    }

    #[test]
    fn windows_drive_types_read_as_network_shares_removable_drives_or_local_disks() {
        // GetDriveTypeW's answers: a mapped drive or a share is remote.
        assert_eq!(drive_kind(4), VolumeKind::Network);
        assert_eq!(drive_kind(2), VolumeKind::Removable);
        assert_eq!(drive_kind(5), VolumeKind::Removable);
        // Fixed, a RAM disk, unknown and no root folder at all.
        for local in [3, 6, 0, 1] {
            assert_eq!(drive_kind(local), VolumeKind::Local, "{local}");
        }
    }

    #[test]
    fn macos_mount_flags_read_as_network_shares_removable_drives_or_local_disks() {
        // An SMB share: no MNT_LOCAL.
        assert_eq!(mount_kind(0x0000_0018), VolumeKind::Network);
        // A memory card: MNT_LOCAL and MNT_REMOVABLE.
        assert_eq!(mount_kind(0x0000_1200), VolumeKind::Removable);
        // The data volume: MNT_LOCAL, journaled, and the like.
        assert_eq!(mount_kind(0x0080_9080), VolumeKind::Local);
    }

    #[test]
    fn linux_file_system_types_read_as_network_shares_removable_drives_or_local_disks() {
        assert_eq!(file_system_kind(0x6969), VolumeKind::Network);
        assert_eq!(file_system_kind(0xFF53_4D42), VolumeKind::Network);
        assert_eq!(file_system_kind(0x4D44), VolumeKind::Removable);
        // ext4, Btrfs, tmpfs.
        for local in [0xEF53, 0x9123_683E, 0x0102_1994] {
            assert_eq!(file_system_kind(local), VolumeKind::Local, "{local:#x}");
        }
    }

    #[test]
    fn the_system_says_a_temporary_folder_is_on_a_local_disk() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(volume_kind(dir.path()), VolumeKind::Local);
        // Missing: taken as local, so that a root away keeps its entries.
        assert_eq!(
            volume_kind(&dir.path().join("missing")),
            VolumeKind::Local
        );
    }

    #[cfg(windows)]
    #[test]
    fn a_network_path_is_a_network_share_by_its_text() {
        assert_eq!(
            volume_kind(Path::new(r"\\server\share\Projects")),
            VolumeKind::Network
        );
        assert_eq!(
            volume_kind(Path::new(r"\\?\UNC\server\share\Projects")),
            VolumeKind::Network
        );
    }
}
