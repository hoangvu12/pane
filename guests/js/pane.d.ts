// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Types for Pane's extension contract, `pane:extension/command` in
// wit/extension.wit, as JavaScript and TypeScript commands see it. They
// describe plain values only; nothing here is specific to the JS engine.
/// <reference path="./wasi.d.ts" />
/// <reference path="./settings.d.ts" />

/** One entry in a command's list view. */
export interface Item {
  /** Passed back to `runAction` or `submitForm` when the user uses the item. */
  id: string;
  title: string;
  /** A second line under the title; omitted or `null` for none. */
  subtitle?: string | null;
  /**
   * When set, activating the item opens this form instead of running
   * `runAction`, and submitting it calls `submitForm`. Omitted or `null` for
   * none.
   */
  form?: Form | null;
  /**
   * The operating systems the item's action (or form) works on; omitted or
   * `null` for every system Pane runs on. Elsewhere Pane still lists the
   * item but shows it as unavailable with the reason, and never calls
   * `runAction` or opens the form for it.
   */
  platforms?: Platform[] | null;
}

/** An operating system Pane runs on. */
export type Platform = "windows" | "macos" | "linux";

/** A command's list view. */
export interface View {
  title: string;
  items: Item[];
}

/** A form the user fills in and submits to the command. */
export interface Form {
  title: string;
  fields: Field[];
  /** The submit button's label. */
  submitLabel: string;
}

export interface Field {
  /** Identifies the field's value in `submitForm`. */
  id: string;
  /** Names the field on screen and to assistive technology. */
  label: string;
  kind: FieldKind;
}

/**
 * A single-line text field, which starts empty, or a choice of exactly one
 * option, whose first option starts chosen.
 */
export type FieldKind =
  | { tag: "text"; val: TextField }
  | { tag: "choice"; val: Choice[] };

export interface TextField {
  /** Shown while the field is empty; omitted or `null` for none. */
  placeholder?: string | null;
}

export interface Choice {
  /** The field's value while this option is chosen. */
  id: string;
  label: string;
}

/** A field's submitted value: its text, or the chosen option's id. */
export interface FieldValue {
  id: string;
  value: string;
}

/**
 * Why a submitted form was not accepted. Throw it from `submitForm` as a
 * plain object: `throw { field: "name", message: "Enter a name" }`.
 */
export interface FormError {
  /** The field the message is about; omitted or `null` for the whole form. */
  field?: string | null;
  message: string;
}

/**
 * An extension command. The module exports it as `command`:
 *
 * ```ts
 * export const command: Command = {
 *   async getView() { ... },
 *   async runAction(id) { ... },
 *   async submitForm(id, values) { ... },
 * };
 * ```
 *
 * Resolving gives Pane the value. Throwing (rejecting) reports an error to the
 * user: from `getView` and `runAction` an `Error`'s message, or a thrown string
 * as is; from `submitForm` a {@link FormError} object. Resolving with a value
 * of the wrong type, such as `undefined` instead of a string, or throwing an
 * `Error` from `submitForm`, is a crash: Pane reports it and starts a fresh
 * instance for the next call.
 */
export interface Command {
  /** Produce the command's list view. */
  getView(): Promise<View>;
  /** Run the action of the item with `itemId`; the text is shown as the result. */
  runAction(itemId: string): Promise<string>;
  /**
   * Handle the submitted form of the item with `itemId`. `values` holds every
   * field of the form, in order. The text is shown as the result; a thrown
   * {@link FormError} is shown next to its field.
   */
  submitForm(itemId: string, values: FieldValue[]): Promise<string>;
}
