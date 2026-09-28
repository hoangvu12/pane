// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Pane's operations sample in TypeScript. Its package publishes the operation
// `greet` (under `operations` in its pane.json), served by
// `publishedOperations` (built with it through package.json's `"pane"`), and
// its command calls the `greet` operation of another package with
// `pane:extension/operations`: a form asks for that package's source (its
// identity, as Pane shows it: `local:` and the folder it was installed
// from), a name, and whether to ask once or twice at once. Items, titles,
// results and errors match the Rust sample (guests/sample-operations) and
// the JavaScript one.
//
// `greet` version 1 takes `{"name": "<name>"}` and answers
// `{"greeting": "Hello, <name>, from TypeScript"}`, or the error "a name is
// needed".
import type {
  Command,
  CustomView,
  FieldValue,
  Form,
  FormError,
  PublishedOperations,
  View,
} from "@pane/extension";
import { call, type CallError } from "pane:extension/operations@0.1.0";

/** `greet`'s input and result, version 1. */
interface GreetInput {
  name: string;
}
interface GreetResult {
  greeting: string;
}

/**
 * Calls `greet` version 1 of the package with `source` for `name`, and
 * returns its greeting. A failed call throws "<kind>: <message>".
 */
async function greet(source: string, name: string): Promise<string> {
  let result: string;
  try {
    const input: GreetInput = { name };
    result = await call(source, "greet", 1, JSON.stringify(input));
  } catch (error) {
    const { kind, message } = (error as { payload: CallError }).payload;
    throw new Error(`${kind}: ${message}`);
  }
  const { greeting } = JSON.parse(result) as Partial<GreetResult>;
  if (typeof greeting !== "string") {
    throw new Error("the answer has no greeting");
  }
  return greeting;
}

const greetForm: Form = {
  title: "Greet through another extension",
  fields: [
    {
      id: "source",
      label: "Package source",
      kind: { tag: "text", val: { placeholder: "local:/path/to/sample-operations" } },
    },
    { id: "name", label: "Name", kind: { tag: "text", val: { placeholder: "TypeScript" } } },
    {
      id: "times",
      label: "Ask",
      kind: {
        tag: "choice",
        val: [
          { id: "once", label: "Once" },
          { id: "twice", label: "Twice at once" },
        ],
      },
    },
  ],
  submitLabel: "Greet",
};

async function getView(): Promise<View> {
  return {
    title: "Call from TypeScript",
    items: [
      {
        id: "greet",
        title: "Greet through another extension",
        subtitle: "Calls its greet operation through Pane",
        form: greetForm,
      },
    ],
  };
}

async function runAction(itemId: string): Promise<string> {
  throw new Error(`unknown item: ${itemId}`);
}

async function submitForm(itemId: string, values: FieldValue[]): Promise<string> {
  if (itemId !== "greet") {
    throw { message: `unknown form: ${itemId}` } satisfies FormError;
  }
  const value = (id: string) => values.find((field) => field.id === id)?.value ?? "";
  const source = value("source").trim();
  if (source === "") {
    throw { field: "source", message: "Enter the package's source" } satisfies FormError;
  }
  const name = value("name");
  try {
    if (value("times") === "twice") {
      // Both calls are made at once; Pane serves them one after another.
      const [first, second] = await Promise.all([greet(source, name), greet(source, name)]);
      return `${first} / ${second}`;
    }
    return await greet(source, name);
  } catch (error) {
    throw { message: (error as Error).message } satisfies FormError;
  }
}

async function openView(itemId: string): Promise<CustomView> {
  throw new Error(`unknown view: ${itemId}`);
}

async function runOperation(operation: string, input: string): Promise<string> {
  if (operation !== "greet") {
    throw new Error(`unknown operation: ${operation}`);
  }
  const { name } = JSON.parse(input) as Partial<GreetInput>;
  if (typeof name !== "string" || name === "") {
    throw new Error("a name is needed");
  }
  const result: GreetResult = { greeting: `Hello, ${name}, from TypeScript` };
  return JSON.stringify(result);
}

export const command: Command = { getView, runAction, submitForm, openView };

export const publishedOperations: PublishedOperations = { runOperation };
