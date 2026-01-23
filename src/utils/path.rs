use std::path::{Path, PathBuf};

pub fn expand_home(path: &str) -> Option<PathBuf> {
    let path = Path::new(path);
    if path.starts_with("~") {
        if path == Path::new("~") {
            dirs::home_dir()
        } else {
            dirs::home_dir().map(|mut h| {
                if h == Path::new("/") {
                    path.strip_prefix("~").unwrap().to_path_buf()
                } else {
                    h.push(path.strip_prefix("~/").unwrap());
                    h
                }
            })
        }
    } else {
        Some(path.to_path_buf())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_expand_home() {
        // No expansion
        assert_eq!(expand_home("/foo/bar").unwrap(), Path::new("/foo/bar"));

        // ~ expansion
        let home = dirs::home_dir().unwrap();
        assert_eq!(expand_home("~").unwrap(), home);

        // ~/path expansion
        assert_eq!(expand_home("~/foo").unwrap(), home.join("foo"));
    }
}
