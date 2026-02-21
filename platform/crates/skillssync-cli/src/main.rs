use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use skillssync_core::{
    watch::SyncWatchStream, McpAgent, ScopeFilter, SkillLifecycleStatus, SkillLocator, SyncEngine,
    SyncTrigger,
};
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(name = "skillssync")]
#[command(about = "SkillsSync multiplaform CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Sync {
        #[arg(long, default_value = "manual")]
        trigger: String,
        #[arg(long)]
        json: bool,
    },
    List {
        #[arg(long, default_value = "all")]
        scope: String,
        #[arg(long)]
        json: bool,
    },
    ListSubagents {
        #[arg(long, default_value = "all")]
        scope: String,
        #[arg(long)]
        json: bool,
    },
    Delete {
        #[arg(long = "skill-key")]
        skill_key: String,
        #[arg(long)]
        confirm: bool,
    },
    Archive {
        #[arg(long = "skill-key")]
        skill_key: String,
        #[arg(long)]
        confirm: bool,
    },
    Restore {
        #[arg(long = "skill-key")]
        skill_key: String,
        #[arg(long)]
        confirm: bool,
    },
    MakeGlobal {
        #[arg(long = "skill-key")]
        skill_key: String,
        #[arg(long)]
        confirm: bool,
    },
    Rename {
        #[arg(long = "skill-key")]
        skill_key: String,
        #[arg(long = "title")]
        title: String,
    },
    Watch,
    Mcp {
        #[command(subcommand)]
        command: McpCommands,
    },
    Doctor,
}

#[derive(Subcommand, Debug)]
enum McpCommands {
    List {
        #[arg(long)]
        json: bool,
    },
    SetEnabled {
        #[arg(long = "server")]
        server: String,
        #[arg(long = "agent")]
        agent: String,
        #[arg(long = "enabled")]
        enabled: bool,
        #[arg(long = "scope")]
        scope: Option<String>,
        #[arg(long = "workspace")]
        workspace: Option<String>,
    },
    Sync,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let engine = SyncEngine::current();

    match cli.command {
        Commands::Sync { trigger, json } => {
            let trigger = SyncTrigger::try_from(trigger.as_str()).map_err(anyhow::Error::msg)?;
            let state = engine.run_sync(trigger)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&state)?);
            } else {
                let skill_conflicts = state.summary.conflict_count;
                let subagent_conflicts = state.subagent_summary.conflict_count;
                println!(
                    "sync={} skills(global={},project={}) subagents(global={},project={}) conflicts={}",
                    format!("{:?}", state.sync.status).to_lowercase(),
                    state.summary.global_count,
                    state.summary.project_count,
                    state.subagent_summary.global_count,
                    state.subagent_summary.project_count,
                    skill_conflicts + subagent_conflicts
                );
            }
        }
        Commands::List { scope, json } => {
            let scope_filter = scope
                .parse::<ScopeFilter>()
                .map_err(|_| anyhow!("unsupported scope: {scope} (all|global|project|archived)"))?;
            let skills = engine.list_skills(scope_filter);
            if json {
                println!("{}", serde_json::to_string_pretty(&skills)?);
            } else {
                for skill in skills {
                    println!(
                        "{}\t{}\t{}\t{}",
                        skill.skill_key,
                        skill.scope,
                        if skill.status == SkillLifecycleStatus::Archived {
                            "archived"
                        } else {
                            "active"
                        },
                        skill.canonical_source_path
                    );
                }
            }
        }
        Commands::ListSubagents { scope, json } => {
            let scope_filter = scope
                .parse::<ScopeFilter>()
                .map_err(|_| anyhow!("unsupported scope: {scope} (all|global|project)"))?;
            let subagents = engine.list_subagents(scope_filter);
            if json {
                println!("{}", serde_json::to_string_pretty(&subagents)?);
            } else {
                for subagent in subagents {
                    println!(
                        "{}\t{}\t{}",
                        subagent.subagent_key, subagent.scope, subagent.canonical_source_path
                    );
                }
            }
        }
        Commands::Delete { skill_key, confirm } => {
            let skill = resolve_skill(&engine, &skill_key, None)?;
            let state = engine.delete(&skill, confirm)?;
            println!("deleted {}; sync={:?}", skill.skill_key, state.sync.status);
        }
        Commands::Archive { skill_key, confirm } => {
            let skill = resolve_skill(&engine, &skill_key, Some(SkillLifecycleStatus::Active))?;
            let state = engine.archive(&skill, confirm)?;
            println!("archived {}; sync={:?}", skill.skill_key, state.sync.status);
        }
        Commands::Restore { skill_key, confirm } => {
            let skill = resolve_skill(&engine, &skill_key, Some(SkillLifecycleStatus::Archived))?;
            let state = engine.restore(&skill, confirm)?;
            println!("restored {}; sync={:?}", skill.skill_key, state.sync.status);
        }
        Commands::MakeGlobal { skill_key, confirm } => {
            let skill = resolve_skill(&engine, &skill_key, Some(SkillLifecycleStatus::Active))?;
            let state = engine.make_global(&skill, confirm)?;
            println!(
                "made-global {}; sync={:?}",
                skill.skill_key, state.sync.status
            );
        }
        Commands::Rename { skill_key, title } => {
            let skill = resolve_skill(&engine, &skill_key, Some(SkillLifecycleStatus::Active))?;
            let state = engine.rename(&skill, &title)?;
            println!("renamed {}; sync={:?}", skill.skill_key, state.sync.status);
        }
        Commands::Watch => {
            let roots = engine.watch_paths();
            let _ = engine.run_sync(SyncTrigger::Manual);
            println!("watching {} roots", roots.len());
            let watcher =
                SyncWatchStream::new(&roots).context("failed to initialize filesystem watcher")?;
            loop {
                match watcher.recv_timeout(Duration::from_secs(2)) {
                    Some(Ok(_event)) => match engine.run_sync(SyncTrigger::AutoFilesystem) {
                        Ok(state) => {
                            let skill_conflicts = state.summary.conflict_count;
                            let subagent_conflicts = state.subagent_summary.conflict_count;
                            println!(
                                "sync ok: global={} project={} conflicts={}",
                                state.summary.global_count,
                                state.summary.project_count,
                                skill_conflicts + subagent_conflicts
                            );
                        }
                        Err(error) => {
                            eprintln!("sync failed: {error}");
                        }
                    },
                    Some(Err(error)) => {
                        eprintln!("watch error: {error}");
                    }
                    None => {}
                }
            }
        }
        Commands::Mcp { command } => match command {
            McpCommands::List { json } => {
                let servers = engine.list_mcp_servers();
                if json {
                    println!("{}", serde_json::to_string_pretty(&servers)?);
                } else {
                    for server in servers {
                        let transport = format!("{:?}", server.transport).to_lowercase();
                        println!(
                            "{}\t{}\t{}\t{}\tcodex={}\tclaude={}\tproject={}",
                            server.server_key,
                            server.scope,
                            server.workspace.unwrap_or_else(|| String::from("-")),
                            transport,
                            server.enabled_by_agent.codex,
                            server.enabled_by_agent.claude,
                            server.enabled_by_agent.project
                        );
                    }
                }
            }
            McpCommands::SetEnabled {
                server,
                agent,
                enabled,
                scope,
                workspace,
            } => {
                let agent = agent.parse::<McpAgent>().map_err(anyhow::Error::msg)?;
                let state = engine.set_mcp_server_enabled(
                    &server,
                    agent,
                    enabled,
                    scope.as_deref(),
                    workspace.as_deref(),
                )?;
                println!(
                    "mcp {} {}={}; sync={:?}",
                    server,
                    agent.as_str(),
                    enabled,
                    state.sync.status
                );
            }
            McpCommands::Sync => {
                let state = engine.run_sync(SyncTrigger::Manual)?;
                println!(
                    "mcp-sync={} servers={} warnings={}",
                    format!("{:?}", state.sync.status).to_lowercase(),
                    state.summary.mcp_count,
                    state.summary.mcp_warning_count
                );
            }
        },
        Commands::Doctor => {
            let env = engine.environment();
            println!("home={}", env.home_directory.display());
            println!("dev_root={}", env.dev_root.display());
            println!("worktrees_root={}", env.worktrees_root.display());
            println!("runtime={}", env.runtime_directory.display());

            let state = engine.load_state();
            println!("state_version={}", state.version);
            println!("skills={}", state.skills.len());
            println!("subagents={}", state.subagents.len());
        }
    }

    Ok(())
}

fn resolve_skill(
    engine: &SyncEngine,
    skill_key: &str,
    status: Option<SkillLifecycleStatus>,
) -> Result<skillssync_core::SkillRecord> {
    engine
        .find_skill(&SkillLocator {
            skill_key: skill_key.to_owned(),
            status,
        })
        .ok_or_else(|| anyhow!("skill not found: {skill_key}"))
}
