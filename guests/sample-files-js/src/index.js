// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Pane's files sample in JavaScript: the same contract as the Files default
// extension (guests/files, Rust) and the TypeScript sample. The user
// chooses a folder in the command's form; Pane lists it with
// `pane:extension/files` (a WASI guest has no folders of its own), and the
// command answers root search with an `open-file` result for each file
// whose name contains every word typed, which Pane opens with the system's
// handler. The folder is kept in the extension's settings.
// @ts-check
import { listFolder } from "pane:extension/files@0.1.0";
import { get, set } from "pane:extension/settings@0.1.0";

/** The settings key holding the chosen folder. */
const FOLDER = "folder";
/** The most files one query lists. */
const MAX_RESULTS = 20;

/** @returns {string | undefined} */
function chosen() {
  return get(FOLDER) || undefined;
}

/**
 * Lists `folder`; a failure throws its reason as an `Error`.
 * @param {string} folder
 */
async function list(folder) {
  try {
    return await listFolder(folder);
  } catch (error) {
    throw new Error(String(/** @type {any} */ (error).payload));
  }
}

/**
 * The last name of `path`, written with `/` or `\` between names.
 * @param {string} path
 */
const lastName = (path) => path.replace(/[\\/]+$/, "").split(/[\\/]/).pop() ?? path;

/** @type {import("@pane/extension").Command} */
export const command = {
  async getView() {
    const folder = chosen();
    return {
      title: "JavaScript files sample",
      items: [
        {
          id: "choose",
          title: "Choose folder",
          subtitle: folder ? `Searching ${folder}` : "No folder chosen yet",
          form: {
            title: "Choose the folder to search",
            fields: [
              { id: FOLDER, label: "Folder", kind: { tag: "text", val: { placeholder: "The folder's full path" } } },
            ],
            submitLabel: "Search this folder",
          },
        },
      ],
    };
  },

  async runAction(itemId) {
    throw new Error(`unknown item: ${itemId}`);
  },

  async submitForm(itemId, values) {
    if (itemId !== "choose") throw { message: `unknown form: ${itemId}` };
    const folder = (values.find((value) => value.id === FOLDER)?.value ?? "").trim();
    if (!folder) throw { field: FOLDER, message: "Enter the folder's full path" };
    let listing;
    try {
      listing = await list(folder);
    } catch (error) {
      throw { field: FOLDER, message: /** @type {Error} */ (error).message };
    }
    set(FOLDER, folder);
    return `Searching “${lastName(folder)}”: ${listing.files.length} files (JavaScript)`;
  },

  async openView(itemId) {
    throw new Error(`unknown view: ${itemId}`);
  },
};

/** @type {import("@pane/extension").RootResults} */
export const rootResults = {
  async resultsFor(query) {
    const folder = chosen();
    if (!folder) return [];
    const listing = await list(folder);
    const words = query.toLowerCase().split(/\s+/).filter(Boolean);
    return listing.files
      .filter((file) => {
        const name = lastName(file.relative).toLowerCase();
        return words.every((word) => name.includes(word));
      })
      .slice(0, MAX_RESULTS)
      .map((file) => ({
        id: file.relative,
        title: lastName(file.relative),
        subtitle: `File in ${lastName(folder)} (JavaScript sample)`,
        action: { tag: "open-file", val: file.path },
      }));
  },
};
