mod git;
mod with;
mod skim;

use {
    structopt::StructOpt,
    std::{
        path::PathBuf,
    }
};

struct CommandExecutionResult {
    output: String,
  }
  
  impl CommandExecutionResult {
    fn empty() -> CommandExecutionResult {
      CommandExecutionResult { output: "".to_string() }
    }
    fn message(message: &str) -> CommandExecutionResult {
      CommandExecutionResult {
        output: message.to_string(),
      }
    }
  }

#[derive(StructOpt)]
enum EvolveCommand {
  Plan {
    onto: git::CommitHash,
    base: git::CommitHash,
    output: PathBuf,
  },
  Execute {
    onto: git::CommitHash,
    base: git::CommitHash
  }
}

impl EvolveCommand {
  fn run(&self, app: &AppOptions) -> anyhow::Result<CommandExecutionResult> {
    use {
      std::io::Write,
    };
    match self {
      EvolveCommand::Plan { onto, base, output } => {
        std::fs::File::create(output)?.write_all(git::EvolvePlan { onto, base }.build()?.as_bytes())?;
      },
      EvolveCommand::Execute { onto, base } => {
          if app.dry_run {
            println!("{}", git::EvolvePlan { onto, base }.build()?);
          } else {
        duct::cmd!("git", "rebase", "-i", onto.to_string())
          .env(
            "GIT_SEQUENCE_EDITOR",
            format!("{} evolve plan {} {}", std::env::current_exe()?.into_os_string().into_string().unwrap(), onto, base)
          )
          .run()?;
        }
      }
    }
    Ok(CommandExecutionResult::empty())
  }
}

#[derive(StructOpt)]
#[structopt(name="trail")]
struct AppOptions {
    #[structopt(short="d")]
    dry_run: bool,
    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(StructOpt)]
enum Command {
    Evolve(EvolveCommand)
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let opts = AppOptions::from_args();

    match opts.cmd {
        Command::Evolve(ref cmd) => { cmd.run(&opts)?; }
    }

    Ok(())
}
