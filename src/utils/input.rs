use anyhow::{Context, Result};
use std::io::{self, Write};

pub fn confirm(prompt: &str, yes: &str, default: Option<&str>) -> Result<bool> {
    print!("{} ", prompt);
    io::stdout().flush()?;

    let mut input = String::new();
    let _ = io::stdin().read_line(&mut input)?;
    let mut input = input.trim_end();
    if input.is_empty() && default.is_some() {
        input = default.context("failed default")?;
    }
    if input == yes {
        Ok(true)
    } else {
        Ok(false)
    }
}
