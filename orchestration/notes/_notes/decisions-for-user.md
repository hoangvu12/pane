# Decisions to put to the user
- #184: ticket says an *added* root on a share/removable drive is left out unless includeOtherVolumes is on; spec stories 49/51/52 read as "adding it is the opt-in". Implemented per the ticket.
- #184: the root volume check was extended to Linux too ("one behaviour"); ticket title says Windows and macOS. Effect: a Linux home on NFS/FUSE, or an added FUSE (ntfs-3g/exfat-fuse) root, is left out by default.
- #133: redaction of user/computer names — spec says substring match >= 3 chars; fix pass changes to word-boundary matching (decision recorded in the doc).
- #192: flush on session end on Linux/macOS signals (fix pass attempts a self-pipe; may be left documented).
