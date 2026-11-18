//! Names of files as fixture data stores them.
//!
//! Fixture trees (a case's starting folders and `expect/tree/`, a workspace's
//! `tree/` and `config/`) live inside this repo. A `.gitignore` or `.gitattributes`
//! among them would apply to the repo itself: the daiku scaffold's `.gitignore`
//! hides `.daiku/active`, so those files would never be committed. Fixture data
//! stores them as `.gitignore.fixture` and `.gitattributes.fixture`, and the runner
//! renames them on the way in and out.

/// File names git reads from any folder of a work tree.
const GIT_FILES: [&str; 2] = [".gitignore", ".gitattributes"];

/// The suffix a git file gets in fixture data.
pub const STORED_SUFFIX: &str = ".fixture";

/// Whether `name` is a git file that fixture data must not hold under its own name.
pub(crate) fn is_git_file(name: &str) -> bool {
    GIT_FILES.contains(&name)
}

/// The real name of a stored file: `.gitignore.fixture` is `.gitignore`.
pub(crate) fn real_name(stored: &str) -> &str {
    match stored.strip_suffix(STORED_SUFFIX) {
        Some(real) if is_git_file(real) => real,
        _ => stored,
    }
}

/// The stored form of a slash-separated relative path.
pub(crate) fn stored_path(rel: &str) -> String {
    rel.split('/')
        .map(|part| {
            if is_git_file(part) {
                format!("{part}{STORED_SUFFIX}")
            } else {
                part.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_files_round_trip() {
        assert_eq!(stored_path(".gitignore"), ".gitignore.fixture");
        assert_eq!(stored_path("a/.gitattributes"), "a/.gitattributes.fixture");
        assert_eq!(stored_path("a/b.txt"), "a/b.txt");
        assert_eq!(real_name(".gitignore.fixture"), ".gitignore");
        assert_eq!(real_name(".gitattributes.fixture"), ".gitattributes");
        assert_eq!(real_name("x.fixture"), "x.fixture");
        assert_eq!(real_name(".gitignore"), ".gitignore");
    }
}
