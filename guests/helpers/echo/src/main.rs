//! `pane-echo`, the helper sample's native helper: a plain program Pane
//! runs for the sample's command, with no dependencies. It reads its input
//! from standard input and answers on standard output, naming the system it
//! was built for, so the answer shows which of the package's files ran.
//!
//! - no arguments: answers `Echoed "<input>" on <system> <processor>`;
//! - `--wait <seconds>`: waits that long first, so that stopping it (by
//!   cancelling, disabling or reloading) can be seen;
//! - `--fail`: writes an explanation to standard error and exits with code 3;
//! - `--flood`: writes more output than Pane passes back (2 MiB).

use std::io::{self, Read, Write};
use std::process::ExitCode;
use std::thread;
use std::time::Duration;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut input = String::new();
    if let Err(error) = io::stdin().read_to_string(&mut input) {
        eprintln!("pane-echo could not read its input: {error}");
        return ExitCode::from(2);
    }
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] => {}
        ["--wait", seconds] => match seconds.parse::<u64>() {
            Ok(seconds) => thread::sleep(Duration::from_secs(seconds)),
            Err(_) => {
                eprintln!("pane-echo: --wait needs a whole number of seconds, not {seconds}");
                return ExitCode::from(2);
            }
        },
        ["--fail"] => {
            eprintln!("pane-echo was asked to fail");
            return ExitCode::from(3);
        }
        ["--flood"] => {
            let line = [b'x'; 1024];
            let mut out = io::stdout().lock();
            for _ in 0..2048 {
                if out.write_all(&line).is_err() {
                    break;
                }
            }
            return ExitCode::SUCCESS;
        }
        other => {
            eprintln!("pane-echo: unknown arguments {other:?}");
            return ExitCode::from(2);
        }
    }
    let system = match std::env::consts::OS {
        "macos" => "macOS",
        "windows" => "Windows",
        "linux" => "Linux",
        other => other,
    };
    let processor = match std::env::consts::ARCH {
        "x86_64" => "x86-64",
        "aarch64" => "arm64",
        other => other,
    };
    print!("Echoed \"{}\" on {system} {processor}", input.trim());
    ExitCode::SUCCESS
}
