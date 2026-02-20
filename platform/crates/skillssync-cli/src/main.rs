use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use skillssync_core::{
    watch::{default_watch_roots, SyncWatchStream},
    ScopeFilter, SkillLifecycleStatus, SkillLocator, SyncEngine, SyncTrigger,
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
    Doctor,
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
                println!(
                    "sync={} global={} project={} conflicts={}",
                    format!("{:?}", state.sync.status).to_lowercase(),
                    state.summary.global_count,
                    state.summary.project_count,
                    state.summary.conflict_count
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
            let roots = default_watch_roots(&engine.environment().home_directory);
            let _ = engine.run_sync(SyncTrigger::Manual);
            println!("watching {} roots", roots.len());
            let watcher =
                SyncWatchStream::new(&roots).context("failed to initialize filesystem watcher")?;
            loop {
                match watcher.recv_timeout(Duration::from_secs(2)) {
                    Some(Ok(_event)) => match engine.run_sync(SyncTrigger::AutoFilesystem) {
                        Ok(state) => {
                            println!(
                                "sync ok: global={} project={} conflicts={}",
                                state.summary.global_count,
                                state.summary.project_count,
                                state.summary.conflict_count
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
        Commands::Doctor => {
            let env = engine.environment();
            println!("home={}", env.home_directory.display());
            println!("dev_root={}", env.dev_root.display());
            println!("worktrees_root={}", env.worktrees_root.display());
            println!("runtime={}", env.runtime_directory.display());

            let state = engine.load_state();
            println!("state_version={}", state.version);
            println!("skills={}", state.skills.len());
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
