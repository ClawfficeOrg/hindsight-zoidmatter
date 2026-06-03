use clap::{Parser, Subcommand, ValueEnum};
use hindsight_api::{add_invariant, list_invariants, remove_invariant};
use hindsight_core::InvariantScope;
use hindsight_missions::InMemoryFactStore;

#[derive(Debug, Clone, ValueEnum)]
enum ScopeKind {
    Global,
    Session,
    Project,
}

#[derive(Parser)]
#[command(name = "zoidmatter", about = "ZoidMatter invariant management CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Manage architectural invariants")]
    Invariant {
        #[command(subcommand)]
        action: InvariantAction,
    },
}

#[derive(Subcommand)]
enum InvariantAction {
    #[command(about = "Register a new architectural invariant")]
    Add {
        #[arg(
            short,
            long,
            help = "A human-readable name (subject key) for the invariant"
        )]
        name: String,

        #[arg(short, long, help = "The invariant content text")]
        text: String,

        #[arg(
            long,
            value_enum,
            default_value = "global",
            help = "Invariant scope: global, session, or project"
        )]
        scope: ScopeKind,

        #[arg(long, help = "Project name (required when --scope project)")]
        project_name: Option<String>,
    },

    #[command(about = "List all architectural invariants")]
    List,

    #[command(about = "Remove an architectural invariant by ID")]
    Remove {
        #[arg(help = "The UUID of the invariant to remove")]
        id: String,
    },
}

fn main() {
    let cli = Cli::parse();

    let store = InMemoryFactStore::default();

    match cli.command {
        Commands::Invariant { action } => match action {
            InvariantAction::Add {
                name,
                text,
                scope,
                project_name,
            } => {
                let invariant_scope = match scope {
                    ScopeKind::Global => InvariantScope::Global,
                    ScopeKind::Session => InvariantScope::Session,
                    ScopeKind::Project => {
                        let pname = project_name.unwrap_or_default();
                        InvariantScope::Project(pname)
                    }
                };

                match add_invariant(&store, &name, &text, invariant_scope) {
                    Ok(item) => {
                        println!("Invariant registered successfully.");
                        println!("  ID:      {}", item.id);
                        println!("  Name:    {}", item.name);
                        println!("  Content: {}", item.content);
                        if let Some(ref s) = item.invariant_scope {
                            println!("  Scope:   {:?}", s);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            InvariantAction::List => match list_invariants(&store) {
                Ok(items) => {
                    if items.is_empty() {
                        println!("No invariants registered.");
                    } else {
                        for item in &items {
                            println!("---");
                            println!("ID:      {}", item.id);
                            println!("Name:    {}", item.name);
                            println!("Content: {}", item.content);
                            if let Some(ref s) = item.invariant_scope {
                                println!("Scope:   {:?}", s);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            },
            InvariantAction::Remove { id } => match remove_invariant(&store, &id) {
                Ok(()) => {
                    println!("Invariant '{}' removed.", id);
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            },
        },
    }
}
