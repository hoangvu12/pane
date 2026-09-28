// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Pane's sample command in JavaScript: a list with one action per item, and a
// form. Items, titles, results and errors match the Rust sample
// (guests/sample-rust) and the TypeScript sample. The JSDoc types let
// TypeScript check this file against Pane's contract; they are optional.
// @ts-check
import { waitFor } from "wasi:clocks/monotonic-clock@0.3.0";
import * as z from "zod/mini";

// Module top-level code runs once, when the component is built, and its state
// is part of the snapshot every instance starts from. Pure setup such as these
// schemas belongs here; secrets, IDs, timestamps and random values do not.
const PORT_RANGE = "port must be between 1 and 65535";
const Settings = z.object({
  name: z.string().check(z.minLength(1, "name must not be empty")),
  port: z.int().check(z.minimum(1, PORT_RANGE), z.maximum(65535, PORT_RANGE)),
});

/** The greeting form's options, by id. */
const GREETINGS = { hello: "Hello", morning: "Good morning", welcome: "Welcome" };

/**
 * The "form" item's form: a name to greet and a greeting to choose.
 * @type {import("@pane/extension").Form}
 */
const GREETING_FORM = {
  title: "Greet someone",
  fields: [
    { id: "name", label: "Name", kind: { tag: "text", val: { placeholder: "Ada Lovelace" } } },
    {
      id: "greeting",
      label: "Greeting",
      kind: {
        tag: "choice",
        val: Object.entries(GREETINGS).map(([id, label]) => ({ id, label })),
      },
    },
  ],
  submitLabel: "Greet",
};

// Checks the submitted values in field order; the first problem is reported.
// Lengths count characters (code points), like the Rust sample.
const Greeting = z.object({
  name: z
    .string()
    .check(
      z.trim(),
      z.minLength(1, "Enter a name"),
      z.refine((name) => [...name].length <= 40, "Use at most 40 characters"),
    ),
  greeting: z.enum(["hello", "morning", "welcome"], "Choose a greeting"),
});

/** @type {import("@pane/extension").Command} */
export const command = {
  async getView() {
    return {
      title: "JavaScript sample",
      items: [
        { id: "greet", title: "Say hello", subtitle: "Answer from the JavaScript guest" },
        { id: "wait", title: "Wait briefly", subtitle: "Await a WASI 0.3 clock, then answer" },
        { id: "validate", title: "Validate settings", subtitle: "Reject settings with an out-of-range port" },
        { id: "random", title: "Roll a number", subtitle: "A random number from this instance" },
        { id: "form", title: "Greet someone", subtitle: "Fill in a form the guest checks", form: GREETING_FORM },
        // Elsewhere Pane lists these as unavailable, says why, and never calls
        // runAction for them.
        { id: "windows-only", title: "Windows-only action", subtitle: "Declared to work on Windows only", platforms: ["windows"] },
        { id: "not-windows", title: "macOS and Linux action", subtitle: "Declared to work on macOS and Linux only", platforms: ["macos", "linux"] },
      ],
    };
  },

  async runAction(itemId) {
    switch (itemId) {
      case "greet":
        return "Hello from the JavaScript guest";
      case "wait":
        // A native component-model async import; the command suspends here.
        await waitFor(50_000_000);
        return "Waited 50 ms inside the JavaScript guest";
      case "validate": {
        const parsed = z.safeParse(Settings, { name: "Pane", port: 70000 });
        if (!parsed.success) {
          // The thrown error's message is shown to the user as the command's error.
          throw new Error(`Invalid settings: ${parsed.error.issues[0].message}`);
        }
        return `Settings are valid: ${parsed.data.name} on port ${parsed.data.port}`;
      }
      case "random":
        return String(Math.random());
      case "windows-only":
        return "Ran the Windows-only action in the JavaScript guest";
      case "not-windows":
        return "Ran the macOS and Linux action in the JavaScript guest";
      default:
        throw new Error(`unknown item: ${itemId}`);
    }
  },

  async submitForm(itemId, values) {
    if (itemId !== "form") {
      throw { message: `unknown form: ${itemId}` };
    }
    const submitted = Object.fromEntries(values.map(({ id, value }) => [id, value]));
    const parsed = z.safeParse(Greeting, submitted);
    if (!parsed.success) {
      // A thrown FormError ({ field, message }) is shown next to its field.
      const issue = parsed.error.issues[0];
      throw { field: String(issue.path[0]), message: issue.message };
    }
    const { name, greeting } = parsed.data;
    return `${GREETINGS[greeting]}, ${name}, from the JavaScript guest`;
  },
};
