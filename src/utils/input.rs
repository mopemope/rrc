use anyhow::Result;
use std::io::{self, Write};

/// Asks a yes/no question on stdin.
///
/// `y` / `yes` (case-insensitive) answers yes, an empty line selects `default`,
/// and anything else answers no. End of input always answers no, so that a
/// closed stdin can never confirm a destructive operation.
pub fn confirm(prompt: &str, default: bool) -> Result<bool> {
    let stdin = io::stdin();
    confirm_with_reader(stdin.lock(), prompt, default)
}

pub fn confirm_with_reader<R: io::BufRead>(
    mut reader: R,
    prompt: &str,
    default: bool,
) -> Result<bool> {
    print!("{prompt} ");
    io::stdout().flush()?;

    let mut input = String::new();
    if reader.read_line(&mut input)? == 0 {
        // EOF
        println!();
        return Ok(false);
    }
    let input = input.trim();
    if input.is_empty() {
        return Ok(default);
    }
    Ok(input.eq_ignore_ascii_case("y") || input.eq_ignore_ascii_case("yes"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confirm_with_reader() {
        assert!(confirm_with_reader("Y\n".as_bytes(), "prompt", false).unwrap());
        assert!(confirm_with_reader("y\n".as_bytes(), "prompt", false).unwrap());
        assert!(confirm_with_reader(" yes \n".as_bytes(), "prompt", false).unwrap());
        assert!(!confirm_with_reader("n\n".as_bytes(), "prompt", true).unwrap());
        assert!(!confirm_with_reader("yeah\n".as_bytes(), "prompt", true).unwrap());
        assert!(confirm_with_reader("\n".as_bytes(), "prompt", true).unwrap());
        assert!(!confirm_with_reader("\n".as_bytes(), "prompt", false).unwrap());
    }

    #[test]
    fn test_confirm_eof_is_no() {
        assert!(!confirm_with_reader("".as_bytes(), "prompt", true).unwrap());
    }
}
