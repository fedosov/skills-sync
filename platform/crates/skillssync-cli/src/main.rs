use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use skillssync_core::{DotagentsScope, SyncEngine};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(name = "skillssync")]
#[command(about = "SkillsSync strict dotagents CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Sync {
        #[arg(long, default_value = "all")]
        scope: String,
        #[arg(long)]
        json: bool,
    },
    Watch {
        #[arg(long, default_value = "all")]
        scope: String,
        #[arg(long, default_value_t = 2)]
        interval_seconds: u64,
    },
    Skills {
        #[command(subcommand)]
        command: SkillsCommands,
    },
    Mcp {
        #[command(subcommand)]
        command: McpCommands,
    },
    MigrateDotagents {
        #[arg(long, default_value = "all")]
        scope: String,
    },
    Doctor,
}

#[derive(Subcommand, Debug)]
enum SkillsCommands {
    Install {
        #[arg(long, default_value = "all")]
        scope: String,
    },
    List {
        #[arg(long, default_value = "all")]
        scope: String,
        #[arg(long)]
        json: bool,
    },
    Add {
        package: String,
        #[arg(long, default_value = "all")]
        scope: String,
    },
    Remove {
        package: String,
        #[arg(long, default_value = "all")]
        scope: String,
    },
    Update {
        package: Option<String>,
        #[arg(long, default_value = "all")]
        scope: String,
    },
}

#[derive(Subcommand, Debug)]
enum McpCommands {
    List {
        #[arg(long, default_value = "all")]
        scope: String,
        #[arg(long)]
        json: bool,
    },
    Add {
        #[arg(required = true)]
        args: Vec<String>,
        #[arg(long, default_value = "all")]
        scope: String,
    },
    Remove {
        #[arg(required = true)]
        args: Vec<String>,
        #[arg(long, default_value = "all")]
        scope: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let engine = SyncEngine::current();

    match cli.command {
        Commands::Sync { scope, json } => {
            let scope = parse_scope(&scope)?;
            engine.run_dotagents_sync(scope)?;
            engine.run_dotagents_install_frozen(scope)?;
            let skills = engine.list_dotagents_skills(scope)?;
            let mcp = engine.list_dotagents_mcp(scope)?;

            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "scope": format!("{scope:?}").to_lowercase(),
                        "skills": skills,
                        "mcp_servers": mcp,
                    }))?
                );
            } else {
                println!(
                    "strict-sync ok scope={} skills={} mcp={}",
                    format!("{scope:?}").to_lowercase(),
                    skills.len(),
                    mcp.len()
                );
            }
        }
        Commands::Watch {
            scope,
            interval_seconds,
        } => {
            let scope = parse_scope(&scope)?;
            let interval = Duration::from_secs(interval_seconds.max(1));
            println!(
                "watching strict dotagents sync, scope={}, interval={}s",
                format!("{scope:?}").to_lowercase(),
                interval.as_secs()
            );
            loop {
                match run_sync_iteration(&engine, scope) {
                    Ok((skills, mcp)) => {
                        println!(
                            "strict-sync ok scope={} skills={} mcp={}",
                            format!("{scope:?}").to_lowercase(),
                            skills,
                            mcp
                        );
                    }
                    Err(error) => {
                        eprintln!("strict-sync failed: {error}");
                    }
                }
                std::thread::sleep(interval);
            }
        }
        Commands::Skills { command } => match command {
            SkillsCommands::Install { scope } => {
                let scope = parse_scope(&scope)?;
                engine.run_dotagents_install_frozen(scope)?;
                println!("skills install completed for scope={}", scope_label(scope));
            }
            SkillsCommands::List { scope, json } => {
                let scope = parse_scope(&scope)?;
                let skills = engine.list_dotagents_skills(scope)?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&skills)?);
                } else {
                    for skill in skills {
                        println!(
                            "{}\t{}\t{}\t{}",
                            skill.skill_key,
                            skill.scope,
                            skill
                                .install_status
                                .unwrap_or_else(|| String::from("unknown")),
                            skill.canonical_source_path
                        );
                    }
                }
            }
            SkillsCommands::Add { package, scope } => {
                let scope = parse_scope(&scope)?;
                engine.run_dotagents_command(scope, &["add", package.as_str()])?;
                println!("skills add completed for scope={}", scope_label(scope));
            }
            SkillsCommands::Remove { package, scope } => {
                let scope = parse_scope(&scope)?;
                engine.run_dotagents_command(scope, &["remove", package.as_str()])?;
                println!("skills remove completed for scope={}", scope_label(scope));
            }
            SkillsCommands::Update { package, scope } => {
                let scope = parse_scope(&scope)?;
                let mut args = vec![String::from("update")];
                if let Some(pkg) = package {
                    args.push(pkg);
                }
                let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
                engine.run_dotagents_command(scope, &refs)?;
                println!("skills update completed for scope={}", scope_label(scope));
            }
        },
        Commands::Mcp { command } => match command {
            McpCommands::List { scope, json } => {
                let scope = parse_scope(&scope)?;
                let servers = engine.list_dotagents_mcp(scope)?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&servers)?);
                } else {
                    for server in servers {
                        println!(
                            "{}\t{}\t{}\t{}",
                            server.server_key,
                            server.scope,
                            server.workspace.unwrap_or_else(|| String::from("-")),
                            format!("{:?}", server.transport).to_lowercase()
                        );
                    }
                }
            }
            McpCommands::Add { args, scope } => {
                let scope = parse_scope(&scope)?;
                let mut command = vec![String::from("mcp"), String::from("add")];
                command.extend(args);
                let refs = command.iter().map(String::as_str).collect::<Vec<_>>();
                engine.run_dotagents_command(scope, &refs)?;
                println!("mcp add completed for scope={}", scope_label(scope));
            }
            McpCommands::Remove { args, scope } => {
                let scope = parse_scope(&scope)?;
                let mut command = vec![String::from("mcp"), String::from("remove")];
                command.extend(args);
                let refs = command.iter().map(String::as_str).collect::<Vec<_>>();
                engine.run_dotagents_command(scope, &refs)?;
                println!("mcp remove completed for scope={}", scope_label(scope));
            }
        },
        Commands::MigrateDotagents { scope } => {
            let scope = parse_scope(&scope)?;
            engine.migrate_to_dotagents(scope)?;
            println!("migration completed for scope={}", scope_label(scope));
        }
        Commands::Doctor => {
            let env = engine.environment();
            println!("home={}", env.home_directory.display());
            println!("dev_root={}", env.dev_root.display());
            println!("worktrees_root={}", env.worktrees_root.display());
            println!("runtime={}", env.runtime_directory.display());

            let user_contract = resolve_user_agents_contract_path(&env.home_directory);
            println!(
                "user_agents_toml={}",
                user_contract
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| String::from("missing"))
            );
        }
    }

    Ok(())
}

fn parse_scope(value: &str) -> Result<DotagentsScope> {
    value
        .parse::<DotagentsScope>()
        .map_err(|_| anyhow!("unsupported scope: {value} (all|user|project)"))
}

fn scope_label(scope: DotagentsScope) -> &'static str {
    match scope {
        DotagentsScope::All => "all",
        DotagentsScope::User => "user",
        DotagentsScope::Project => "project",
    }
}

fn run_sync_iteration(engine: &SyncEngine, scope: DotagentsScope) -> Result<(usize, usize)> {
    engine.run_dotagents_sync(scope)?;
    engine.run_dotagents_install_frozen(scope)?;
    let skills = engine.list_dotagents_skills(scope)?;
    let mcp = engine.list_dotagents_mcp(scope)?;
    Ok((skills.len(), mcp.len()))
}

fn resolve_user_agents_contract_path(home_directory: &Path) -> Option<PathBuf> {
    let primary = home_directory.join(".agents").join("agents.toml");
    if primary.exists() {
        return Some(primary);
    }

    let legacy = home_directory
        .join(".config")
        .join("ai-agents")
        .join("agents.toml");
    if legacy.exists() {
        return Some(legacy);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::resolve_user_agents_contract_path;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_temp_home(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "skillssync-cli-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create temp home");
        root
    }

    #[test]
    fn resolve_user_agents_contract_prefers_primary_path() {
        let home = make_temp_home("primary");
        let primary = home.join(".agents").join("agents.toml");
        let legacy = home.join(".config").join("ai-agents").join("agents.toml");
        fs::create_dir_all(primary.parent().expect("parent")).expect("create primary dir");
        fs::create_dir_all(legacy.parent().expect("parent")).expect("create legacy dir");
        fs::write(&primary, "[skills]\n").expect("write primary");
        fs::write(&legacy, "[skills]\n").expect("write legacy");

        let resolved = resolve_user_agents_contract_path(&home).expect("resolve path");
        assert_eq!(resolved, primary);

        fs::remove_dir_all(home).expect("cleanup");
    }

    #[test]
    fn resolve_user_agents_contract_falls_back_to_legacy_path() {
        let home = make_temp_home("legacy");
        let legacy = home.join(".config").join("ai-agents").join("agents.toml");
        fs::create_dir_all(legacy.parent().expect("parent")).expect("create legacy dir");
        fs::write(&legacy, "[skills]\n").expect("write legacy");

        let resolved = resolve_user_agents_contract_path(&home).expect("resolve path");
        assert_eq!(resolved, legacy);

        fs::remove_dir_all(home).expect("cleanup");
    }
}
