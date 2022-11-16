mod asm;
mod term;
mod vm;

use crate::asm::assemble;
use anyhow::{bail, Context, Result};
use clap::{Parser, ValueEnum};
use crossterm::{
    execute,
    terminal::{Clear, ClearType},
};
use std::{
    fmt::{Debug, Display},
    io::stdout,
    path::PathBuf,
};
use term::TerminalExt;
use vm::VM;

fn hex_to_bytes(s: String) -> Result<Vec<u8>> {
    fn parse_line(s: &str, res: &mut Vec<u8>) -> Result<()> {
        let mut s = s.trim_start();
        if let Some(index) = s.find(';') {
            s = &s[..index];
        }
        let s = s.trim_end().to_ascii_lowercase();
        if s.len() % 2 != 0
            || s.chars()
                .any(|c| !('0'..='9').contains(&c) && !('a'..='f').contains(&c))
        {
            bail!("Invalid hex string: {s}");
        }
        res.extend(
            (0..s.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()),
        );
        Ok(())
    }
    let mut res = Vec::new();
    for (i, line) in s.split('\n').enumerate() {
        parse_line(line, &mut res).context(format!("On line {i}"))?;
    }
    Ok(res)
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum InputFormat {
    Assembly,
    Hex,
    Binary,
}

impl Display for InputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            InputFormat::to_possible_value(self).unwrap().get_name()
        )
    }
}

#[derive(Parser, Debug)]
#[command(name = "v8-cpu")]
#[command(author = "Mivik")]
#[command(version = "0.1")]
#[command(about = "An interactive terminal UI to simulate v8-cpu programs", long_about = None)]
struct Args {
    file: PathBuf,

    /// The format of the input file
    #[arg(short, long, value_name = "format", default_value_t = InputFormat::Assembly)]
    format: InputFormat,

    /// Enable quiet mode, only outputing the final result
    #[arg(short, long)]
    quiet: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let bytes = std::fs::read(&args.file)
        .context(format!("Failed to read file from {}", args.file.display()))?;
    let bytes = match args.format {
        InputFormat::Assembly => {
            let s = String::from_utf8(bytes).context("Failed to parse input as string")?;
            assemble(&s).context("Failed to assemble")?
        }
        InputFormat::Hex => {
            let s = String::from_utf8(bytes).context("Failed to parse input as string")?;
            hex_to_bytes(s).context("Failed to decode hex string")?
        }
        InputFormat::Binary => bytes,
    };
    if bytes.len() > 256 {
        bail!("Input bytecode is too large (> 256)");
    }
    let mut vm = VM::new();
    vm.fill(&bytes);
    if args.quiet {
        execute!(stdout(), Clear(ClearType::All))?;
        while vm.step()? {}
        vm.print_state()
    } else {
        vm.interactive()
    }
}
