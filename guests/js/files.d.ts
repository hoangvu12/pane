// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Declarations for `pane:extension/files` in wit/files.wit: the files of a
// folder, which Pane lists for the extension under its bounded scan policy,
// since a WASI guest has no folders to read.

/** `pane:extension/files@0.1.0`. */
declare module "pane:extension/files@0.1.0" {
  /** One file Pane found. */
  export interface FoundFile {
    /** Its absolute path, as the system writes it: what an `open-file` result takes. */
    path: string;
    /** Its path below the folder listed, with `/` between names, such as `notes/plan.md`. */
    relative: string;
  }

  /** What Pane found in a folder. */
  export interface FolderListing {
    /** The files, breadth first, each folder's entries in name order. */
    files: FoundFile[];
    /** Pane stopped at one of its limits before it had looked at everything. */
    truncated: boolean;
  }

  /**
   * Lists the regular files in `folder`, an absolute path, and its
   * subfolders: at most 8 folders deep, 5,000 files and 20,000 entries
   * looked at, without hidden entries, links or names that are not Unicode;
   * a subfolder Pane cannot read is skipped. On failure (a relative path, a
   * missing folder, a file, a folder Pane may not read) the promise rejects
   * with an object whose `payload` is the reason. Pane stops the listing
   * when it cancels the call waiting for it.
   */
  export function listFolder(folder: string): Promise<FolderListing>;
}
