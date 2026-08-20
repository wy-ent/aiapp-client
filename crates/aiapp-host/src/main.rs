//! aiapp-host: aiapp desktop/CLI host container (Phase 2 open-source part).
//!
//! Runs `.aiapp` unified application packages in desktop/CLI environments:
//! - `info` / `validate`: inspect and validate the package (manifest + WASM + permissions);
//! - `run`: load the package and execute it after the permission gate
//!   (`--exec wasmtime` enables real WASM execution);
//! - `capabilities`: list the WIT host capability catalog (ecosystem rules).
//!
//! Paired with `aiapp-engine` (the runtime library): this binary is one concrete
//! form of "host". Other forms (host App / standalone App / browser renderer)
//! implement the same WIT interface to reuse the application.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use aiapp_engine::host::Host;
use aiapp_engine::{Runtime, RuntimeConfig};
use aiapp_format::{AiappPackage, ValidationReport};

mod desktop_host;
#[cfg(feature = "wasmtime")]
use aiapp_engine::WasmExec;

#[derive(Parser)]
#[command(
    name = "aiapp-host",
    version,
    about = "aiapp desktop/CLI host container: runs .aiapp unified application packages"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Show application package info (manifest + validation report + permission summary)
    Info {
        /// `.aiapp` package directory
        pkg: PathBuf,
    },
    /// Validate the application package
    Validate {
        /// `.aiapp` package directory
        pkg: PathBuf,
    },
    /// Run the application package
    Run {
        /// `.aiapp` package directory
        pkg: PathBuf,
        /// Permissions granted by the host (comma-separated; default = all declared in the manifest)
        #[arg(long, value_delimiter = ',')]
        grant: Vec<String>,
        /// Data directory (where `save-data`/`load-data` land; default `./host-data/<app_id>`)
        #[arg(long)]
        data_dir: Option<PathBuf>,
        /// Execution mode: `meta` (default, metadata/lifecycle) or `wasmtime` (real WASM execution, requires the feature)
        #[arg(long, default_value = "meta")]
        exec: String,
    },
    /// List the WIT host capability catalog
    Capabilities,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = run(cli);
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Info { pkg } => cmd_info(&pkg),
        Command::Validate { pkg } => cmd_validate(&pkg),
        Command::Run {
            pkg,
            grant,
            data_dir,
            exec,
        } => cmd_run(&pkg, &grant, data_dir.as_deref(), &exec),
        Command::Capabilities => cmd_capabilities(),
    }
}

/// Parse the `.aiapp` package.
fn parse_pkg(pkg: &PathBuf) -> Result<AiappPackage> {
    AiappPackage::parse(pkg)
        .with_context(|| format!("failed to parse application package {}", pkg.display()))
}

/// Print the validation report.
fn print_report(report: &ValidationReport, label: &str) {
    println!(
        "[{label}] validation result: {}",
        if report.ok { "passed" } else { "failed" }
    );
    for e in &report.errors {
        println!("  ✗ {e}");
    }
    for w in &report.warnings {
        println!("  ⚠ {w}");
    }
}

/// info: print application package info.
fn cmd_info(pkg: &PathBuf) -> Result<()> {
    let package = parse_pkg(pkg)?;
    let m = &package.manifest;
    println!("app: {} ({})", m.name, m.app_id);
    println!(
        "  version: {}   category: {}   template: {}",
        m.version, m.category, m.template
    );
    println!("  description: {}", m.description);
    println!("  host SDK: {}   entry: {}", m.host_sdk_version, m.entry);
    println!("  platforms: {}", m.platforms.join(", "));
    println!(
        "  permissions: {}",
        if m.permissions.is_empty() {
            "(none)".to_string()
        } else {
            m.permissions.join(", ")
        }
    );
    println!("  WASM: {} bytes", package.wasm.len());
    if !package.resources.is_empty() {
        println!("  resources: {} item(s)", package.resources.len());
    }
    let report = aiapp_format::validate_package(&package.dir.clone().unwrap_or_else(|| pkg.clone()));
    print_report(&report, "info");
    Ok(())
}

/// validate: validation only.
fn cmd_validate(pkg: &PathBuf) -> Result<()> {
    let report = aiapp_format::validate_package(pkg);
    print_report(&report, "validate");
    if report.ok {
        Ok(())
    } else {
        anyhow::bail!("application package validation failed")
    }
}

/// capabilities: list the WIT host capability catalog.
fn cmd_capabilities() -> Result<()> {
    println!("WIT host capability catalog (contract v{}):", aiapp_format::WIT_VERSION);
    println!("{:-<48}", "");
    for (name, desc) in aiapp_format::HOST_CAPABILITIES {
        println!("  {:<16} {}", name, desc);
    }
    println!("{:-<48}", "");
    println!("Corresponding permission declarations: strings in manifest.permissions (storage / notifications / log)");
    Ok(())
}

/// run: load and run the application package.
fn cmd_run(pkg: &PathBuf, grant: &[String], data_dir: Option<&std::path::Path>, exec: &str) -> Result<()> {
    let package = parse_pkg(pkg)?;
    let m = &package.manifest;

    // Data directory: default <data_dir>/<app_id>, ensuring app data isolation and portability
    let base = data_dir.unwrap_or_else(|| std::path::Path::new("./host-data"));
    let app_data_dir = base.join(&m.app_id);

    match exec {
        "meta" => {
            // meta mode requires a tokio runtime to drive the engine (Host trait is async)
            let rt = tokio::runtime::Runtime::new()
                .context("failed to initialize tokio runtime")?;
            rt.block_on(run_meta(&package, grant, &app_data_dir))
        }
        "wasmtime" => run_wasmtime(&package, grant, &app_data_dir),
        other => anyhow::bail!("unknown execution mode: {other} (options: meta / wasmtime)"),
    }
}

/// meta mode: lightweight execution via aiapp-engine (validation + permission gate + lifecycle hooks).
async fn run_meta(
    package: &AiappPackage,
    grant: &[String],
    app_data_dir: &std::path::Path,
) -> Result<()> {
    let host: Arc<dyn Host> = Arc::new(desktop_host::DesktopHost::new(app_data_dir));
    host.log(
        "info",
        &format!("host ready, data directory: {}", app_data_dir.display()),
    )
    .await;

    let config = RuntimeConfig {
        granted_permissions: if grant.is_empty() {
            None
        } else {
            Some(grant.to_vec())
        },
    };
    let runtime = Runtime::with_config(host.clone(), config);
    let app = runtime
        .run(package)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    let m = &app.manifest;
    println!("[run] app {} v{} started successfully (lightweight mode)", m.name, m.version);
    println!("[run] permission summary:");
    for perm in &m.permissions {
        use aiapp_engine::permissions::Permission;
        match app.permissions.check(perm) {
            Permission::Granted => println!("    {perm}: granted"),
            Permission::NotDeclared => println!("    {perm}: not declared"),
            Permission::Denied => println!("    {perm}: denied"),
        }
    }
    Ok(())
}

/// wasmtime mode: real WASM execution (requires the wasmtime feature at compile time).
#[cfg(feature = "wasmtime")]
fn run_wasmtime(package: &AiappPackage, _grant: &[String], app_data_dir: &std::path::Path) -> Result<()> {
    let host: Arc<dyn Host> = Arc::new(desktop_host::DesktopHost::new(app_data_dir));
    let exec = WasmExec::new(host);
    let summary = exec
        .run(&package.wasm, &package.manifest.app_id)
        .map_err(|e| anyhow::anyhow!(e))?;
    println!("[run] {summary}");
    Ok(())
}

/// Placeholder message when wasmtime mode is not compiled (no feature).
#[cfg(not(feature = "wasmtime"))]
fn run_wasmtime(_package: &AiappPackage, _grant: &[String], _app_data_dir: &std::path::Path) -> Result<()> {
    anyhow::bail!(
        "wasmtime execution mode requires the feature enabled: cargo build -p aiapp-host --features wasmtime"
    )
}
