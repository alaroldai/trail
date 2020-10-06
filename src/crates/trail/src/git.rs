use {
  anyhow::{ anyhow, Error, Result },
  duct,
  log::{ trace },
  std::{
    collections::{HashMap, HashSet},
    fmt::{self, Formatter, Display},
    io::{self, Read, BufRead},
    process::{Command, Stdio},
    str::{FromStr},
  },
  rayon::prelude::*,
  serde::{Deserialize, Serialize},
};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub struct Branch(String);
impl FromStr for Branch {
  type Err = Error;
  fn from_str(s: &str) -> Result<Self> {
    if Command::new("git").args(&["show-ref", "--verify", &format!("refs/heads/{}", s)]).stdout(Stdio::null()).status()?.success() {
      return Ok(Branch(s.to_string()))
    } 

    io::BufReader::new(duct::cmd!("git", "branch", "--points-at", s, "--format", "%(refname:short)").reader()?)
      .lines()
      .next()
      .unwrap()
      .map(Branch)
      .map_err(|e| anyhow!("Failed to parse branch: {}", e))
  }
}
impl Display for Branch {
  fn fmt(&self, f: &mut Formatter) -> fmt::Result {
    self.0.fmt(f)
  }
}

impl AsRef<String> for Branch {
  fn as_ref(&self) -> &String {
    &self.0
  }
}

impl Into<String> for Branch {
  fn into(self) -> String {
    self.0
  }
}

#[derive(Default, Debug, PartialEq, Eq, Hash, Clone, Serialize, Deserialize)]
pub struct CommitHash(String);
impl FromStr for CommitHash {
  type Err = Error;
  fn from_str(s: &str) -> Result<Self> {
    duct::cmd!("git", "rev-parse", "--verify", s)
      .read()
      .map(|s| s.trim().to_string())
      .map(CommitHash)
      .map_err(|_| anyhow!("Failed to parse commit"))
  }
}

impl From<Branch> for CommitHash {
  fn from(branch: Branch) -> Self {
    branch.0.parse().unwrap()
  }
}

impl Display for CommitHash {
  fn fmt(&self, f: &mut Formatter) -> fmt::Result {
    self.0.fmt(f)
  }
}

impl AsRef<String> for CommitHash {
  fn as_ref(&self) -> &String {
    &self.0
  }
}

impl Into<String> for CommitHash {
  fn into(self) -> String {
    self.0
  }
}
impl CommitHash {
  pub fn branches(&self) -> Result<Vec<Branch>> {
    Ok(
      io::BufReader::new(
        duct::cmd!(
          "git",
          "branch",
          "--points-at",
          &self.0,
          "--format",
          "%(refname:short)"
        )
        .reader()?,
      )
      .lines()
      .filter_map(|b| b.ok().and_then(|b| Branch::from_str(&b).ok()))
      .collect::<Vec<Branch>>()
    )
  }

  pub fn get_patch_id(&self) -> Result<String> {
    use duct::cmd;

    trace!("get_patch_id");
  
    cmd!("git", "show", &self.0)
      .pipe(cmd!("git", "patch-id"))
      .read()?
      .split_whitespace()
      .next()
      .ok_or_else(|| anyhow!("Couldn't read patch-id"))
      .map(String::from)
  }

  pub fn get_name_rev(&self) -> Result<String> {
    Ok(
      String::from_utf8(
        Command::new("git")
          .arg("name-rev")
          .arg(&self.0)
          .output()?
          .stdout,
      )?
      .trim()
      .to_string(),
    )
  }

  pub fn read_trailers(&self) -> Result<HashMap<String, String>> {
    trace!("read_trailers");
    let format_patch = Command::new("git")
      .arg("show")
      .arg("--format=email")
      .arg("--no-patch")
      .arg(self.as_ref())
      .stdout(Stdio::piped())
      .spawn()?;
    let mut interpret_trailers = Command::new("git")
      .arg("interpret-trailers")
      .arg("--parse")
      .stdin(Stdio::from(
        format_patch
          .stdout
          .ok_or_else(|| anyhow!("Couldn't unwrap format_patch.stdout"))?,
      ))
      .stdout(Stdio::piped())
      .spawn()?;

    interpret_trailers.try_wait()?;

    let mut outs = String::default();
    interpret_trailers
      .stdout
      .ok_or_else(|| anyhow!("Couldn't unwrap interpret_trailers.stdout"))?
      .read_to_string(&mut outs)?;

    let mut trailers: HashMap<String, String> = std::collections::HashMap::new();
    for line in outs.lines() {
      let sp: Vec<_> = line.split(": ").collect();

      trailers.insert(sp[0].to_owned(), sp[1..].join(": "));
    }

    Ok(trailers)
  }

  pub fn get_short_hash(&self) -> Result<String> {
    trace!("running get_short_hash");
    Ok(
      String::from_utf8(
        Command::new("git")
          .arg("rev-parse")
          .arg("--short")
          .arg(self.0.trim())
          .output()?
          .stdout,
      )?
      .trim()
      .to_string(),
    )
  }

  pub fn get_commit_message_short(&self) -> Result<String> {
    trace!("running get_commit_message_short");
    Ok(
      String::from_utf8(
        Command::new("git")
          .arg("log")
          .arg("--format=%s")
          .arg("-n")
          .arg("1")
          .arg(self.0.trim())
          .output()?
          .stdout,
      )?
      .trim()
      .to_string(),
    )
  }

  pub fn branches_containing_revision(&self) -> Result<Vec<Branch>> {
    Ok(
      io::BufReader::new(
        duct::cmd!(
          "git",
          "branch",
          "--contains",
          &self.0,
          "--format",
          "%(refname:short)"
        )
        .reader()?,
      )
      .lines()
      .flat_map(|s| s)
      .filter_map(|s| if s.contains(" detached ") { None } else { Some(Branch::from_str(&s).unwrap()) })
      .collect()
    )
  }
}

pub fn add_trailer(key: &str, value: &str) -> Result<()> {
  let mut git = Command::new("git")
    .arg("-c")
    .arg(format!(
      "core.editor=git interpret-trailers --in-place --trailer=\"{}: {}\"",
      key, value
    ))
    .arg("commit")
    .arg("--amend")
    .arg("-q")
    .spawn()?;
  git.try_wait()?;
  Ok(())
}

pub fn branches() -> Result<Vec<String>> {
  let git = Command::new("git").arg("branch").output()?;
  Ok(String::from_utf8(git.stdout)?.lines().map(str::to_string).collect())
}

pub fn get_branches_containing_head() -> Result<Vec<String>> {
  let git = Command::new("git")
    .args(&["branch", "--list", "--contains", "HEAD", "--format", "%(refname:short)"])
    .output()?;
  Ok(String::from_utf8(git.stdout)?.lines().map(str::to_string).collect())
}

pub fn get_merge_base(branch: &CommitHash, into_branch: &CommitHash) -> Result<CommitHash> {
  let git = Command::new("git")
    .args(&["merge-base", into_branch.as_ref(), branch.as_ref()])
    .output()?;
  Ok(String::from_utf8(git.stdout)?.trim().parse()?)
}

pub fn checkout(revision: &str) -> Result<()> {
  Command::new("git").arg("checkout").arg(revision).spawn()?.wait()?;
  Ok(())
}

pub fn multi_merge_base<'a>(branches: impl Iterator<Item=&'a CommitHash>) -> Result<CommitHash> {
  let git = Command::new("git").args(&["merge-base", "--octopus"]).args(branches.map(|r| r.as_ref())).output()?;
  Ok(String::from_utf8(git.stdout)?.trim().parse()?)
}

pub fn get_changes_between<'a>(
  first: &CommitHash,
  second: &CommitHash,
  git_args: impl Into<Option<&'a [&'a str]>>,
) -> Result<Vec<CommitHash>> {
  let git_args = git_args.into().unwrap_or(&["--format=%H"]);
  let mut cmd = Command::new("git");
    cmd.arg("log");
    cmd.arg("--no-merges");
    cmd.args(git_args);
    cmd.arg(format!("{}..{}", first.as_ref(), second.as_ref()));
  let out_string = String::from_utf8(cmd.output()?.stdout)?;
  Ok(
    dbg!(out_string
      .split('\n')
      .filter(|s| !s.trim().is_empty())
      .filter_map(|s| s.parse().ok())
      .collect(),
  ))
}

pub fn get_commits_affecting_files(
  start: &CommitHash,
  end: &CommitHash,
  fileset: &HashSet<String>,
) -> Result<Vec<CommitHash>> {
  let log = std::str::from_utf8(
    &Command::new("git")
      .arg("log")
      .args(&[
        &format!("{}..{}", start, end),
        "--format=\"%H\"",
      ])
      .output()?
      .stdout,
  )?
  .trim()
  .lines()
  .map(|s| s.trim_matches('"'))
  .map(String::from)
  .collect::<Vec<String>>();

  Ok(
    log
      .par_iter()
      .map(String::as_str)
      .filter(|hash| {
        match || -> Result<bool> {
          let files = std::str::from_utf8(
            &Command::new("git")
              .arg("diff-tree")
              .args(&["--name-only", "-r", hash])
              .output()?
              .stdout,
          )?
          .trim()
          .lines()
          .map(String::from)
          .collect::<Vec<String>>();

          Ok(
            files
              .into_iter()
              .collect::<HashSet<String>>()
              .intersection(&fileset)
              .count()
              != 0,
          )
        }() {
          Ok(result) => result,
          Err(_) => false,
        }
      })
      .filter_map(|s| s.parse().ok())
      .collect(),
  )
}

pub fn show_range_diff(base: &CommitHash, first: &CommitHash, second: &CommitHash) -> Result<()> {
  use duct::cmd;

  cmd!("git", "range-diff", base.as_ref(), first.as_ref(), second.as_ref()).run()?;
  Ok(())
}

pub fn get_repo_root() -> Result<String> {
  Ok(
    std::str::from_utf8(
      &Command::new("git")
        .arg("rev-parse")
        .arg("--show-toplevel")
        .output()?
        .stdout,
    )?
    .trim()
    .to_string(),
  )
}

pub struct EvolvePlan<'a> {
  pub onto: &'a CommitHash,
  pub base: &'a CommitHash,
}
impl<'a> EvolvePlan<'a> {
  pub fn build(self) -> Result<String> {
    use std::collections::BTreeSet;
    let mut heads = self.base.branches_containing_revision()?.into_iter().collect::<BTreeSet<_>>();
    let excl = &self.onto.branches_containing_revision()?.into_iter().collect::<BTreeSet<_>>();
    heads = heads.difference(&excl).cloned().collect();

    let commits: Vec<Vec<CommitHash>> = io::BufReader::new(
      duct::cmd("git", {
        let mut args: Vec<String> = 
          ["rev-list", "--reverse", "--topo-order", "--parents"]
            .iter().map(|s| String::from(*s)).collect();
        args.append(&mut heads.iter().map(Branch::to_string).collect());
        args.push(format!("^{}", self.base));
        args
      })
      .reader()?,
    )
    .lines()
    .map(|s| {
      s.unwrap()
        .split(char::is_whitespace)
        .map(|s| CommitHash::from_str(&String::from(s)).unwrap())
        .collect()
    })
    .collect();

    let mut last_picked = self.base.clone();

    let mut commands: Vec<String> = Default::default();

    macro_rules! push_cmd {
      ($cmd:literal, $arg:expr) => {
        commands.push(format!("{} {}", $cmd, $arg));
      };
    }

    macro_rules! label { ($s:expr) => { push_cmd!("label", $s); } }
    macro_rules! pick { ($s:expr) => { push_cmd!("pick", format!("{} {}", $s, $s.get_commit_message_short()?)); } }
    macro_rules! reset { ($s:expr) => { push_cmd!("reset", $s); } }
    macro_rules! branch { ($s:expr) => { push_cmd!("exec", format!("git branch -f {}", $s)); } }

    label!(self.base);
    for line in commits.into_iter() {
      let (hash, parent) = (&line[0], &line[1]);
      if parent != &last_picked {
        reset!(parent);
      }
      pick!(hash);
      label!(hash);
      for branch in hash.branches()? {
        branch!(branch);
      }
      last_picked = hash.clone();
    }
    reset!(self.base);

    Ok(commands.join("\n"))
  }
}