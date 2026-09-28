// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Pane's files sample in TypeScript: the same contract as the Files default
// extension (guests/files, Rust) and the JavaScript sample. The user
// chooses a folder in the command's form; Pane lists it with
// `pane:extension/files` (a WASI guest has no folders of its own), and the
// command answers root search with an `open-file` result for each file
// whose name contains every word typed, which Pane opens with the system's
// handler. The folder is kept in the extension's settings.
import { type FolderListing, listFolder } from "pane:extension/files@0.1.0";
import { get, set } from "pane:extension/settings@0.1.0";
import type { Command, CustomView, FieldValue, RootResult, RootResults, View } from "@pane/extension";

/** The settings key holding the chosen folder. */
const FOLDER = "folder";
/** The most files one query lists. */
const MAX_RESULTS = 20;

function chosen(): string | undefined {
  return get(FOLDER) || undefined;
}

/** Lists `folder`; a failure throws its reason as an `Error`. */
async function list(folder: string): Promise<FolderListing> {
  try {
    return await listFolder(folder);
  } catch (error) {
    throw new Error(String((error as { payload: unknown }).payload));
  }
}

/** The last name of `path`, written with `/` or `\` between names. */
const lastName = (path: string): string => path.replace(/[\\/]+$/, "").split(/[\\/]/).pop() ?? path;

async function getView(): Promise<View> {
  const folder = chosen();
  return {
    title: "TypeScript files sample",
    items: [
      {
        id: "choose",
        title: "Choose folder",
        subtitle: folder ? `Searching ${folder}` : "No folder chosen yet",
        form: {
          title: "Choose the folder to search",
          fields: [{ id: FOLDER, label: "Folder", kind: { tag: "text", val: { placeholder: "The folder's full path" } } }],
          submitLabel: "Search this folder",
        },
      },
    ],
  };
}

async function runAction(itemId: string): Promise<string> {
  throw new Error(`unknown item: ${itemId}`);
}

async function submitForm(itemId: string, values: FieldValue[]): Promise<string> {
  if (itemId !== "choose") throw { message: `unknown form: ${itemId}` };
  const folder = (values.find((value) => value.id === FOLDER)?.value ?? "").trim();
  if (!folder) throw { field: FOLDER, message: "Enter the folder's full path" };
  let listing: FolderListing;
  try {
    listing = await list(folder);
  } catch (error) {
    throw { field: FOLDER, message: (error as Error).message };
  }
  set(FOLDER, folder);
  return `Searching “${lastName(folder)}”: ${listing.files.length} files (TypeScript)`;
}

async function openView(itemId: string): Promise<CustomView> {
  throw new Error(`unknown view: ${itemId}`);
}

export const command: Command = { getView, runAction, submitForm, openView };

export const rootResults: RootResults = {
  async resultsFor(query: string): Promise<RootResult[]> {
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
        subtitle: `File in ${lastName(folder)} (TypeScript sample)`,
        action: { tag: "open-file", val: file.path },
      }));
  },
};
