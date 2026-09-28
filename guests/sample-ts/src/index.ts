// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Pane's sample command in TypeScript: a list with one action per item, and a
// form. Items, titles, results and errors match the Rust sample
// (guests/sample-rust) and the JavaScript sample.
import type { Command, FieldValue, Form, FormError, View } from "@pane/extension";
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

/** The "form" item's form: a name to greet and a greeting to choose. */
const GREETING_FORM: Form = {
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

async function getView(): Promise<View> {
  return {
    title: "TypeScript sample",
    items: [
      { id: "greet", title: "Say hello", subtitle: "Answer from the TypeScript guest" },
      { id: "wait", title: "Wait briefly", subtitle: "Await a WASI 0.3 clock, then answer" },
      { id: "validate", title: "Validate settings", subtitle: "Reject settings with an out-of-range port" },
      { id: "random", title: "Roll a number", subtitle: "A random number from this instance" },
      { id: "form", title: "Greet someone", subtitle: "Fill in a form the guest checks", form: GREETING_FORM },
    ],
  };
}

async function runAction(itemId: string): Promise<string> {
  switch (itemId) {
    case "greet":
      return "Hello from the TypeScript guest";
    case "wait":
      // A native component-model async import; the command suspends here.
      await waitFor(50_000_000);
      return "Waited 50 ms inside the TypeScript guest";
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
    default:
      throw new Error(`unknown item: ${itemId}`);
  }
}

async function submitForm(itemId: string, values: FieldValue[]): Promise<string> {
  if (itemId !== "form") {
    throw { message: `unknown form: ${itemId}` } satisfies FormError;
  }
  const submitted = Object.fromEntries(values.map(({ id, value }) => [id, value]));
  const parsed = z.safeParse(Greeting, submitted);
  if (!parsed.success) {
    // A thrown FormError is shown next to its field.
    const issue = parsed.error.issues[0];
    throw { field: String(issue.path[0]), message: issue.message } satisfies FormError;
  }
  const { name, greeting } = parsed.data;
  return `${GREETINGS[greeting]}, ${name}, from the TypeScript guest`;
}

export const command: Command = { getView, runAction, submitForm };
