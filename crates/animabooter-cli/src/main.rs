use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;

use animabooter_core::{AppError, CancelToken, DriveInfo, EventSink, PipelineResult};
use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "animabooter",
    version,
    about = "Flash USB drives safely from the terminal"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// List removable drives available for flashing.
    List(ListArgs),
    /// Flash an image to a device. Verification is enabled by default.
    Flash(FlashArgs),
    /// Request safe removal of a device.
    Eject(DeviceArgs),
}

#[derive(Debug, Args)]
struct ListArgs {
    /// Include non-removable devices. Use only when you understand the risk.
    #[arg(long)]
    unsafe_mode: bool,
    /// Print machine-readable JSON instead of a table.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct FlashArgs {
    /// Source image, optionally compressed with gzip, xz, zstd or bzip2.
    image: String,
    /// Target device path, for example /dev/sdb.
    #[arg(long)]
    device: String,
    /// Required acknowledgement that all data on the target will be destroyed.
    #[arg(long)]
    yes: bool,
    /// Include non-removable devices in validation.
    #[arg(long)]
    unsafe_mode: bool,
    /// Skip read-back verification.
    #[arg(long)]
    no_verify: bool,
    /// Print the final result as JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct DeviceArgs {
    /// Device path, for example /dev/sdb.
    #[arg(long)]
    device: String,
}

#[derive(Clone, Default)]
struct TerminalSink;

impl EventSink for TerminalSink {
    fn phase(&self, phase: animabooter_core::Phase) {
        eprintln!("phase: {phase:?}");
    }

    fn progress(&self, payload: &animabooter_core::ProgressPayload) {
        let percent = payload
            .percent
            .map(|value| format!(" {value:.1}%"))
            .unwrap_or_default();
        eprintln!(
            "write: {} bytes{} at {:.1} MiB/s",
            payload.written, percent, payload.speed_mbs
        );
    }

    fn verify_progress(&self, payload: &animabooter_core::VerifyPayload) {
        eprintln!("verify: {:.1}%", payload.percent);
    }

    fn log(&self, line: String) {
        eprintln!("{line}");
    }

    fn error(&self, payload: &animabooter_core::ErrorPayload) {
        eprintln!("error: {}", payload.message);
        if let Some(hint) = &payload.hint {
            eprintln!("hint: {hint}");
        }
    }

    fn done(&self, _payload: &animabooter_core::DonePayload) {}
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            if let Some(hint) = error.hint() {
                eprintln!("hint: {hint}");
            }
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<(), AppError> {
    match Cli::parse().command {
        Command::List(args) => list_drives(args),
        Command::Flash(args) => flash(args),
        Command::Eject(args) => eject(args),
    }
}

fn list_drives(args: ListArgs) -> Result<(), AppError> {
    let drives = animabooter_core::list_drives(args.unsafe_mode)?;
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&drives)
                .map_err(|error| AppError::config(error.to_string()))?
        );
    } else {
        print_drive_table(&drives);
    }
    Ok(())
}

fn print_drive_table(drives: &[DriveInfo]) {
    if drives.is_empty() {
        println!("No eligible drives found.");
        return;
    }

    println!("PATH\tSIZE\tMODEL\tREMOVABLE");
    for drive in drives {
        println!(
            "{}\t{}\t{} {}\t{}",
            drive.path,
            format_bytes(drive.size_bytes),
            drive.vendor.trim(),
            drive.model.trim(),
            drive.removable
        );
    }
}

fn flash(args: FlashArgs) -> Result<(), AppError> {
    if !args.yes {
        return Err(AppError::Safety {
            message: "flashing requires explicit confirmation with --yes".to_string(),
            hint: Some(format!("This will destroy all data on {}.", args.device)),
        });
    }

    let cancel = CancelToken::new();
    let cancel_for_handler = cancel.clone();
    ctrlc::set_handler(move || cancel_for_handler.cancel())
        .map_err(|error| AppError::platform(format!("cannot install Ctrl+C handler: {error}")))?;

    let sink: Arc<dyn EventSink> = Arc::new(TerminalSink);
    let result = animabooter_core::flash::run_flash(
        &args.image,
        &args.device,
        !args.no_verify,
        args.unsafe_mode,
        cancel,
        sink,
    )?;

    print_flash_result(&result, args.json)
}

fn print_flash_result(result: &PipelineResult, json: bool) -> Result<(), AppError> {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(result)
                .map_err(|error| AppError::config(error.to_string()))?
        );
    } else {
        println!(
            "Finished: {} bytes written in {:.1}s ({:.1} MiB/s). Verified: {}. SHA-256: {}",
            result.total_bytes,
            result.elapsed_secs,
            result.avg_speed_mbs,
            result.verified,
            result.sha256
        );
    }
    Ok(())
}

fn eject(args: DeviceArgs) -> Result<(), AppError> {
    animabooter_core::eject(Path::new(&args.device))
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}
