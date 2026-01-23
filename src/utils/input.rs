use anyhow::{Context, Result};
use std::io::{self, Write};

pub fn confirm(prompt: &str, yes: &str, default: Option<&str>) -> Result<bool> {
    let stdin = io::stdin();
    confirm_with_reader(stdin.lock(), prompt, yes, default)
}

pub fn confirm_with_reader<R: io::BufRead>(
    mut reader: R,
    prompt: &str,
    yes: &str,
    default: Option<&str>,
) -> Result<bool> {
    print!("{} ", prompt);
    io::stdout().flush()?;

    let mut input = String::new();
    let _ = reader.read_line(&mut input)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confirm_with_reader() {
        assert!(confirm_with_reader("Y\n".as_bytes(), "prompt", "Y", None).unwrap());
        assert!(!confirm_with_reader("n\n".as_bytes(), "prompt", "Y", None).unwrap());
        assert!(confirm_with_reader("\n".as_bytes(), "prompt", "Y", Some("Y")).unwrap());
        assert!(!confirm_with_reader("\n".as_bytes(), "prompt", "Y", Some("n")).unwrap());
    }
}
