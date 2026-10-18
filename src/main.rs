use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use theme_report::{
    agent, cache,
    inputs::{Config, PROFILE},
    model::Report,
    rules,
};

#[derive(Parser)]
#[command(version, about = "Read-only Quattro theme health reports")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}
#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Text,
    Json,
}
#[derive(Subcommand)]
enum Commands {
    #[command(group(clap::ArgGroup::new("target").required(true).args(["current", "path"])))]
    Audit {
        #[arg(long, conflicts_with = "path")]
        current: bool,
        path: Option<PathBuf>,
        #[arg(long, required_unless_present = "current", value_parser = [PROFILE])]
        profile: Option<String>,
        #[arg(long, value_enum, default_value = "text")]
        format: Format,
        #[arg(long)]
        force: bool,
        /// Override report cache (never the active theme).
        #[arg(long)]
        cache_dir: Option<PathBuf>,
        /// Omarchy runtime root (defaults to OMARCHY_PATH, then /usr/share/omarchy).
        #[arg(long)]
        omarchy_root: Option<PathBuf>,
    },
    Explain {
        #[arg(value_parser = clap::builder::PossibleValuesParser::new(rules::REGISTRY.iter().map(|r| r.0).chain(["advisory.lua-syntax", "advisory.user-shell"])))]
        rule_id: String,
    },
    Rules,
    Version,
    /// Show which Omarchy runtime behaviors this analyzer can verify.
    Runtime {
        /// Omarchy runtime root (defaults to OMARCHY_PATH, then /usr/share/omarchy).
        #[arg(long)]
        omarchy_root: Option<PathBuf>,
    },
    /// Print a prompt for your agent, built from the latest current-theme report.
    Prompt {
        #[command(flatten)]
        handoff: HandoffArgs,
    },
    /// Hand the latest report to your Omarchy default agent.
    Agent {
        #[command(flatten)]
        handoff: HandoffArgs,
        /// Open the agent in a new terminal window (default).
        #[arg(long, conflicts_with = "copy")]
        window: bool,
        /// Copy the prompt to the clipboard instead of launching.
        #[arg(long)]
        copy: bool,
    },
    /// Print what the panel shows between audits, as JSON: the published report, the
    /// active theme name, the default agent and the saved review. Reads only; never audits.
    #[command(hide = true)]
    State {
        /// Whose saved review to read (defaults to the published report's theme).
        #[arg(long)]
        theme: Option<String>,
        /// Report cache (defaults to $XDG_CACHE_HOME/theme-report).
        #[arg(long)]
        cache_dir: Option<PathBuf>,
    },
    #[command(hide = true)]
    DecodeImage {
        path: PathBuf,
    },
}
#[derive(clap::Args)]
struct HandoffArgs {
    /// fix: problems and cautions · measure: checks that could not be measured · explain: no edits · review: a thorough read-only review, saved as Markdown.
    #[arg(value_parser = ["fix", "measure", "explain", "review"])]
    kind: String,
    /// Limit the prompt to one check.
    #[arg(long)]
    rule: Option<String>,
    /// Use this report JSON instead of the latest current-theme report.
    #[arg(long)]
    report: Option<PathBuf>,
    /// Report cache (defaults to $XDG_CACHE_HOME/theme-report).
    #[arg(long)]
    cache_dir: Option<PathBuf>,
    /// Omarchy runtime root (defaults to OMARCHY_PATH, then /usr/share/omarchy).
    #[arg(long)]
    omarchy_root: Option<PathBuf>,
}
fn default_cache(home: &std::path::Path) -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".cache"))
        .join("theme-report")
}
/// Build the prompt for a handoff and write the report snapshot it cites.
fn handoff(args: &HandoffArgs) -> Result<(agent::Env, agent::Prompt, Report)> {
    let home = PathBuf::from(std::env::var_os("HOME").context("HOME is not set")?);
    let cache = args
        .cache_dir
        .clone()
        .unwrap_or_else(|| default_cache(&home));
    let env = agent::Env::from_process(resolve_omarchy(args.omarchy_root.clone()), cache)?;
    // An explicit --report may be a link the user chose; the published report never is.
    let path = match &args.report {
        Some(p) => p.canonicalize().context("Cannot find that report file")?,
        None => env.cache.join("current.json"),
    };
    let bytes = theme_report::inputs::bounded(&path, theme_report::state::REPORT_LIMIT)
        .with_context(|| "No report yet. Run theme-report audit --current first.")?;
    let report: Report =
        serde_json::from_slice(&bytes).context("The report file is not a supported report")?;
    if report.schema_version != 2 {
        bail!("Unsupported report format")
    }
    if args.report.is_none() {
        let active =
            theme_report::state::first_line(&home.join(".local/state/omarchy/current/theme.name"))
                .unwrap_or_default();
        if !active.is_empty() && active != report.theme {
            bail!(
                "The last report is for {}, but the current theme is {}. Run theme-report audit --current first.",
                display(&report.theme),
                display(&active)
            )
        }
    }
    let kind = agent::Kind::parse(&args.kind).context("Unknown prompt kind")?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    // Validate before writing anything, so a refused prompt leaves no handoff file.
    agent::selected(&report, kind, args.rule.as_deref())?;
    agent::workspace(&report, &env)?;
    let file = agent::write_handoff(&env, &report, kind, now.as_millis() as u64)?;
    let prompt = agent::prompt(
        &report,
        kind,
        args.rule.as_deref(),
        &env,
        &file,
        &agent::date_stamp(now.as_secs()),
    )?;
    Ok((env, prompt, report))
}
fn display(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_control() {
                c.escape_default().to_string()
            } else {
                c.to_string()
            }
        })
        .collect()
}

/// Closing the panel terminates the helper; remove its scratch files before exiting.
fn cleanup_on_termination() {
    #[cfg(target_os = "linux")]
    unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        for signal in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
            libc::sigaddset(&mut set, signal);
        }
        // Blocked before any other thread exists, so only the waiter receives them.
        // Child processes start with an empty mask (std resets it on spawn).
        if libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut()) != 0 {
            return;
        }
        std::thread::spawn(move || {
            let mut signal = 0;
            if libc::sigwait(&set, &mut signal) == 0 {
                theme_report::cache::cleanup_staging();
                libc::_exit(128 + signal);
            }
        });
    }
}
fn resolve_omarchy(explicit: Option<PathBuf>) -> PathBuf {
    explicit.unwrap_or_else(|| {
        std::env::var_os("OMARCHY_PATH")
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/usr/share/omarchy"))
    })
}

fn main() {
    // Behave like other CLI tools when piped into `head`: exit quietly, never panic.
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
    let cli = Cli::parse();
    let json = matches!(
        &cli.command,
        Commands::Audit {
            format: Format::Json,
            ..
        }
    );
    if matches!(&cli.command, Commands::Audit { .. }) {
        cleanup_on_termination();
    }
    let result = run(cli);
    theme_report::cache::cleanup_staging();
    if let Err(e) = result {
        if json {
            println!(
                "{}",
                serde_json::json!({"schemaVersion":2,"state":"error","error":format!("{e:#}")})
            );
        } else {
            eprintln!("Theme Report: {}", display(&format!("{e:#}")));
        }
        std::process::exit(1)
    }
}
fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::State { theme, cache_dir } => {
            let home = PathBuf::from(std::env::var_os("HOME").context("HOME is not set")?);
            let cache = cache_dir.unwrap_or_else(|| default_cache(&home));
            let env = agent::Env::from_process(resolve_omarchy(None), cache)?;
            println!(
                "{}",
                theme_report::state::panel_state(&env, theme.as_deref())
            );
        }
        Commands::DecodeImage { path } => {
            let result = match theme_report::wallpaper::decode(&path) {
                Ok(samples) => serde_json::json!({"samples":samples}),
                Err(e) => serde_json::json!({"error":e.to_string()}),
            };
            println!("{result}");
        }
        Commands::Prompt { handoff: args } => {
            let (_, prompt, _) = handoff(&args)?;
            print!("{}", prompt.text);
        }
        Commands::Agent {
            handoff: args,
            window: _,
            copy,
        } => {
            let (env, prompt, report) = handoff(&args)?;
            let chosen = agent::default_agent(&env);
            if copy {
                agent::copy_to_clipboard(&prompt.text)?;
                println!("copied");
            } else if chosen.is_none() {
                agent::pick_agent()?;
                println!("no-agent");
            } else {
                if args.kind == "review" {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    agent::begin_review(&env, &report, chosen.as_deref().unwrap_or_default(), now)?;
                }
                agent::launch_window(prompt.workspace.workdir(), &prompt.text)?;
                println!("launched");
            }
        }
        Commands::Version => println!(
            "theme-report {} · rules {} · profile {PROFILE}",
            env!("CARGO_PKG_VERSION"),
            rules::RULESET_VERSION
        ),
        Commands::Runtime { omarchy_root: root } => {
            let root = resolve_omarchy(root);
            let files = theme_report::inputs::runtime_files(&root);
            let contract = theme_report::contract::Contract::evaluate(&files);
            println!("Omarchy runtime: {}", display(&root.to_string_lossy()));
            println!(
                "Status: {}\n",
                match contract.status {
                    "reviewed" => "reviewed (exact runtime files)",
                    "compatible" => "compatible (unrelated runtime changes only)",
                    "partial" => "partial (some checks paused)",
                    _ => "unsupported (checks paused)",
                }
            );
            for (label, ok) in [
                ("Palette loading and core colors", contract.palette),
                ("Terminal selection fallbacks", contract.selection),
                ("Shell surfaces and control states", contract.shell),
            ] {
                println!("  {label:<36} {}", if ok { "verified" } else { "paused" });
            }
            if !contract.unverified.is_empty() {
                println!("\nUnreviewed changes: {}", contract.unverified.join(", "));
            }
            println!("\nExtract hashes:");
            for (name, digest) in theme_report::contract::extract_hashes(&files) {
                println!("  {name:<20} {digest}");
            }
        }
        Commands::Rules => {
            for (id, cat, budget, mandatory, blocking) in rules::REGISTRY {
                println!(
                    "{id:<34} {cat:<12} {budget:>2} points{}{}",
                    if mandatory { " · mandatory" } else { "" },
                    if blocking { " · blocking" } else { "" }
                );
            }
            for id in ["advisory.lua-syntax", "advisory.user-shell"] {
                println!("{id:<34} {:<12}  0 points · advisory", "validity");
            }
        }
        Commands::Explain { rule_id } => {
            if rule_id == "advisory.lua-syntax" {
                println!(
                    "advisory.lua-syntax\n\nCategory: validity\nBudget: 0 points (advisory)\n\nNo dialect-matched non-executing validator is bundled. Theme Lua is never executed for an audit. Validate separately with a trusted validator matching the installed dialect."
                );
                return Ok(());
            }
            if rule_id == "advisory.user-shell" {
                println!(
                    "advisory.user-shell\n\nCategory: validity\nBudget: 0 points (advisory)\n\nInvalid values in your machine-level ~/.config/omarchy/shell.toml. They are reported but never counted against the theme; checks that depend on them stay unassessed."
                );
                return Ok(());
            }
            let row = rules::REGISTRY
                .iter()
                .find(|r| r.0 == rule_id)
                .context("Unknown rule ID; use theme-report rules")?;
            println!(
                "{}\n\nCategory: {}\nBudget: {} points\nMandatory: {}\nBlocking: {}",
                row.0, row.1, row.2, row.3, row.4
            );
            if let Some(policy) = include_str!("../rules/v1.md")
                .lines()
                .find(|line| line.starts_with(&format!("| {} |", row.0)))
            {
                println!("\nRule | Budget | Pass | Caution | Failure / incomplete\n{policy}");
            }
            println!(
                "\nCaution loses half the budget; failure loses its full budget. Unsupported inputs are unassessed, never a pass. See the bundled rules/v1.md for applicability and ownership."
            );
        }
        Commands::Audit {
            current,
            path,
            profile,
            format,
            force,
            cache_dir,
            omarchy_root,
        } => {
            if !current && path.is_none() {
                bail!("Choose --current or a materialized theme path")
            }
            if !current && profile.as_deref() != Some(PROFILE) {
                bail!("Path audits require --profile {PROFILE}")
            }
            let home = PathBuf::from(std::env::var_os("HOME").context("HOME is not set")?);
            let cache_dir = cache_dir.unwrap_or_else(|| {
                std::env::var_os("XDG_CACHE_HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| home.join(".cache"))
                    .join("theme-report")
            });
            let theme = path.unwrap_or_else(|| home.join(".local/state/omarchy/current/theme"));
            let config = Config {
                theme,
                current,
                home,
                omarchy: resolve_omarchy(omarchy_root),
                cache: cache_dir,
                profile,
            };
            let report = cache::audit(&config, force);
            // Scratch is not needed for output; remove it before writing, since a closed
            // pipe (e.g. `| head`) ends the process mid-print.
            theme_report::cache::cleanup_staging();
            let report = report?;
            match format {
                Format::Json => println!("{}", serde_json::to_string_pretty(&report)?),
                Format::Text => print_report(&report),
            }
        }
    }
    Ok(())
}
fn print_report(r: &Report) {
    println!(
        "{} · {}",
        display(&r.theme),
        r.grade.as_deref().unwrap_or("Not graded")
    );
    if r.state == "provisional" {
        println!("Provisional · some contexts could not be checked");
    }
    println!("{}\n", display(&r.summary));
    for c in &r.categories {
        println!(
            "  {:<12} {:>11}{} · {}",
            c.id,
            c.grade.as_deref().unwrap_or("Not checked"),
            if c.state == "limited" {
                " · Limited"
            } else {
                ""
            },
            display(&c.summary)
        );
    }
    println!();
    for f in &r.findings {
        if f.severity != "pass" {
            println!(
                "  {} · {}\n    {}",
                f.severity,
                f.rule_id,
                display(&f.summary)
            );
            if let Some(reason) = f.evidence.get("reason").and_then(|r| r.as_str()) {
                println!("    {}", display(reason));
            }
        }
    }
    for reason in &r.cap_reasons {
        println!("  Grade capped: {}", display(reason));
    }
    println!(
        "\nRead-only · rules {} · profile {}",
        r.ruleset_version, r.profile
    );
}
