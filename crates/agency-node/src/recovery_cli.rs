//! `agency root-recovery`, `root-recover`, and `root-cancel-recovery`: an org's recovery key, kept only as
//! 24 written words (docs/protocol.md, "Recovery key"). The words are read from the terminal, never from an
//! argument, so they stay out of the shell's history, and the key they make is used for one signature and
//! dropped.
use agency_core::recovery::{self, Recovering};
use agency_core::{orgs, Node};
use agency_proto::{AccountKey, Id};
use anyhow::{bail, Context, Result};
use bip39::Mnemonic;
use clap::Args;
use std::io::{BufRead, IsTerminal, Write};

/// How many of the words the owner types back before the recovery key is named in the org.
const WORDS_TYPED_BACK: [usize; 3] = [3, 11, 20];

/// Gives an org a recovery key and shows its 24 words once (on the root key's node). With `--replace`, starts
/// replacing the recovery key with a new one, signed with the current words
#[derive(Args)]
pub struct RootRecovery {
    #[arg(long)]
    org: String,
    /// Replaces the recovery key that the org has; asks for its words first
    #[arg(long, conflicts_with = "same_words_as")]
    replace: bool,
    /// Uses the recovery words of this other org instead of new ones, after you type them: one set of words
    /// for both, so whoever has them can start a recovery of both (each still waits 72 hours)
    #[arg(long, value_name = "OTHER_ORG")]
    same_words_as: Option<String>,
}

/// Starts a recovery of an org with its recovery words, naming a new root key made on this node; with
/// `--finish`, finishes the recovery that waits, once this node has held it for 72 hours
#[derive(Args)]
pub struct RootRecover {
    #[arg(long)]
    org: String,
    #[arg(long)]
    finish: bool,
}

/// Cancels a recovery of an org that waits, by signing the next version with the root key (on the root
/// key's node)
#[derive(Args)]
pub struct RootCancelRecovery {
    #[arg(long)]
    org: String,
}

fn id(raw: &str) -> Result<Id> {
    Id::parse(raw).map_err(anyhow::Error::msg)
}

/// The 24 words of `key`'s seed: the BIP-39 English list, whose last word carries a checksum.
pub fn words_of(key: &AccountKey) -> Result<Vec<String>> {
    let words = Mnemonic::from_entropy(&key.seed())
        .map_err(|why| anyhow::anyhow!("cannot make the recovery words: {why}"))?;
    let list: Vec<String> = words.words().map(str::to_owned).collect();
    if list.len() != 24 {
        bail!(
            "cannot make the recovery words: {} instead of 24",
            list.len()
        );
    }
    Ok(list)
}

/// The key 24 words stand for. A mistyped or misplaced word fails the checksum.
pub fn key_of(words: &str) -> Result<AccountKey> {
    let normal = words
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    let mnemonic = Mnemonic::parse_normalized(&normal)
        .map_err(|why| anyhow::anyhow!("these are not 24 recovery words: {why}"))?;
    let seed: [u8; 32] = mnemonic
        .to_entropy()
        .try_into()
        .map_err(|_| anyhow::anyhow!("these are not 24 recovery words"))?;
    Ok(AccountKey::from_seed(seed))
}

/// Turns the terminal's echo off while it lives, and back on when it is dropped, an error or a panic
/// included. Nothing happens when standard input is not a terminal (a pipe from a password manager).
struct NoEcho {
    is_off: bool,
}

impl NoEcho {
    fn start() -> Self {
        let is_terminal = std::io::stdin().is_terminal();
        let is_off = is_terminal && stty("-echo");
        Self { is_off }
    }
}

impl Drop for NoEcho {
    fn drop(&mut self) {
        if self.is_off {
            stty("echo");
            eprintln!();
        }
    }
}

fn stty(setting: &str) -> bool {
    std::process::Command::new("stty")
        .arg(setting)
        .stdin(std::process::Stdio::inherit())
        .status()
        .is_ok_and(|status| status.success())
}

/// Reads one line without showing it, after a prompt on standard error.
fn read_secret(prompt: &str) -> Result<String> {
    eprint!("{prompt}");
    std::io::stderr().flush()?;
    let _quiet = NoEcho::start();
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    Ok(line.trim().to_owned())
}

fn ask_words(what: &str) -> Result<AccountKey> {
    key_of(&read_secret(&format!(
        "The {what} recovery words, separated by spaces (not shown): "
    ))?)
}

/// The terminal's alternate screen while it lives, so that the words leave no trace in the scrollback once
/// it is dropped. Nothing happens when standard output is not a terminal.
struct AlternateScreen {
    is_on: bool,
}

impl AlternateScreen {
    fn start() -> Self {
        let is_on = std::io::stdout().is_terminal();
        if is_on {
            print!("\x1b[?1049h\x1b[H");
            let _ = std::io::stdout().flush();
        }
        Self { is_on }
    }
}

impl Drop for AlternateScreen {
    fn drop(&mut self) {
        if self.is_on {
            print!("\x1b[?1049l");
            let _ = std::io::stdout().flush();
        }
    }
}

/// Shows `key`'s words and has the owner type a few back, so that words written down wrong are found now.
fn show_words(key: &AccountKey) -> Result<()> {
    let list = words_of(key)?;
    let _screen = AlternateScreen::start();
    println!("Write these 24 words down, on paper, and keep them away from this machine:\n");
    for (number, word) in list.iter().enumerate() {
        println!("{:>2}. {word}", number + 1);
    }
    println!("\nThey are shown only now. Whoever has them can start a recovery of the org.");
    for number in WORDS_TYPED_BACK {
        let typed = read_secret(&format!("Word {number} (not shown): "))?;
        let shown = list.get(number - 1).map(String::as_str).unwrap_or_default();
        if !typed.eq_ignore_ascii_case(shown) {
            bail!("word {number} is not what was shown; nothing was changed, so run this again");
        }
    }
    Ok(())
}

pub async fn root_recovery(node: &Node, args: RootRecovery) -> Result<()> {
    let org = id(&args.org)?;
    if let Some(other) = &args.same_words_as {
        let other = id(other)?;
        let words = ask_words(&format!("{other} org's"))?;
        let doc = recovery::add_same_as(node, &org, &other, &words)?;
        println!(
            "Org {org} has the recovery key of {other} (version {}): the same words recover both.",
            doc.body.version
        );
        return Ok(());
    }
    let new = AccountKey::generate();
    if !args.replace {
        show_words(&new)?;
        let doc = recovery::add(node, &org, new.public())?;
        println!(
            "Org {org} has a recovery key (version {}).",
            doc.body.version
        );
        return Ok(());
    }
    let current = ask_words("current")?;
    show_words(&new)?;
    let doc = recovery::start(node, &org, &current, Recovering::RecoveryKey(new.public())).await?;
    print_started(node, &org, &doc)
}

fn print_started(node: &Node, org: &Id, doc: &agency_proto::org::SignedOrg) -> Result<()> {
    let at = orgs::recovery_takes_effect(node, doc)
        .map(agency_proto::clock::format)
        .context("the start was taken but its time was not noted")?;
    println!(
        "A recovery of {org} was started (version {}). Finish it after {at} with `agency root-recover --org {org} --finish`; until then, the root cancels it by signing any version.",
        doc.body.version
    );
    Ok(())
}

pub async fn root_recover(node: &Node, args: RootRecover) -> Result<()> {
    let org = id(&args.org)?;
    let words = ask_words("org's")?;
    if args.finish {
        let doc = recovery::finish(node, &org, &words).await?;
        println!(
            "The recovery of {org} took effect: the root key is {} (version {}).",
            doc.body.root.public_key.fingerprint(),
            doc.body.version
        );
        return Ok(());
    }
    let doc = recovery::start(node, &org, &words, Recovering::RootKey).await?;
    print_started(node, &org, &doc)
}

pub fn root_cancel_recovery(node: &Node, args: RootCancelRecovery) -> Result<()> {
    let org = id(&args.org)?;
    let doc = recovery::cancel(node, &org)?;
    println!(
        "The recovery of {org} was cancelled (version {}).",
        doc.body.version
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_stand_for_the_key_and_a_wrong_word_fails_the_checksum() {
        let key = AccountKey::from_seed([0x41; 32]);
        let words = words_of(&key).unwrap().join(" ");
        assert_eq!(words.split(' ').count(), 24);
        assert_eq!(key_of(&words).unwrap().public(), key.public());
        let spaced = format!("  {}  ", words.to_uppercase().replace(' ', "   "));
        assert_eq!(
            key_of(&spaced).unwrap().public(),
            key.public(),
            "case and spacing"
        );

        let mut list: Vec<&str> = words.split(' ').collect();
        list.swap(0, 1);
        assert!(key_of(&list.join(" ")).is_err(), "two words swapped");
        assert!(key_of(&words.split(' ').take(23).collect::<Vec<_>>().join(" ")).is_err());
    }
}
