//! The pull request template `daiku pr` starts a body from (N-0019): where it comes
//! from, and how the ticket's links go into it.

use std::io::Read;
use std::path::Path;

use serde::Serialize;

use crate::outcome::Outcome;
use crate::{Error, Warning};

/// Where a template's links go, alone on a line: each such line is replaced by the
/// marked block.
pub const LINKS_PLACEHOLDER: &str = "{{links}}";
/// The start of the links block in a rendered body, alone on a line. GitHub doesn't
/// show it.
pub const LINKS_OPEN: &str = "<!-- daiku:links -->";
/// The end of the links block, alone on a line.
pub const LINKS_CLOSE: &str = "<!-- /daiku:links -->";
/// The workspace's template, and where the embedded default would be written.
pub const WORKSPACE_TEMPLATE: &str = "build/05_ship/templates/pull-request.md";
/// Largest repo PR template we will read: 64 KiB.
pub const MAX_REPO_TEMPLATE_BYTES: usize = 64 * 1024;

/// Which template a body started from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TemplateOrigin {
    /// A template file in the build's repo checkout.
    Repo,
    /// The workspace's own `build/05_ship/templates/pull-request.md`.
    Workspace,
    /// The copy embedded in the binary, used when the workspace's file is missing.
    Default,
}

impl TemplateOrigin {
    /// The word for the origin, as `--json` and `daiku pr --template` show it.
    pub fn as_str(self) -> &'static str {
        match self {
            TemplateOrigin::Repo => "repo",
            TemplateOrigin::Workspace => "workspace",
            TemplateOrigin::Default => "default",
        }
    }
}

/// A chosen template with the ticket's links in place. `path` is relative to the
/// repo for a `repo` template and to the workspace otherwise; for `default` it's
/// the path the file would have.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PrTemplate {
    pub origin: TemplateOrigin,
    pub path: String,
    pub body: String,
}

/// Puts `links` into `text`, which is a template or a body an agent filled in. The
/// markers and `{{links}}` count only when alone on their line (surrounding spaces
/// ignored); a mention in prose or backticks is ordinary text.
///
/// 1. a marked block (a `<!-- daiku:links -->` line, then a `<!-- /daiku:links -->`
///    line) has its contents replaced; with several, the last one is refreshed;
/// 2. else every `{{links}}` line is replaced by the marked block;
/// 3. else the marked block is appended after a blank line.
///
/// The text outside the block is kept as it is. Empty `links` add no markers: a
/// placeholder line is emptied and nothing else changes. The result ends with one
/// newline.
pub fn render_body(text: &str, links: &str) -> String {
    let links = links.trim_matches('\n');
    let marked = || format!("{LINKS_OPEN}\n{links}\n{LINKS_CLOSE}");
    let out = if let Some((head, tail)) = split_block(text) {
        if links.is_empty() {
            text.to_string()
        } else {
            format!("{head}{}{tail}", marked())
        }
    } else if text.lines().any(|l| l.trim() == LINKS_PLACEHOLDER) {
        let block = if links.is_empty() {
            String::new()
        } else {
            marked()
        };
        text.split_inclusive('\n')
            .map(|l| {
                if l.trim() == LINKS_PLACEHOLDER {
                    format!("{block}{}", &l[l.trim_end_matches(['\n', '\r']).len()..])
                } else {
                    l.to_string()
                }
            })
            .collect()
    } else if links.is_empty() {
        text.to_string()
    } else {
        let body = text.trim_end_matches('\n');
        if body.is_empty() {
            marked()
        } else {
            format!("{body}\n\n{}", marked())
        }
    };
    format!("{}\n", out.trim_end_matches('\n'))
}

/// The text before the last marked block and the text after it. The block runs
/// from the start of its opening line to the end of its closing line's content, so
/// the line ending that follows stays in the second part.
fn split_block(text: &str) -> Option<(&str, &str)> {
    let (mut open, mut found, mut at) = (None, None, 0);
    for line in text.split_inclusive('\n') {
        let content = line.trim_end_matches(['\n', '\r']);
        if content.trim() == LINKS_OPEN {
            open = Some(at);
        } else if content.trim() == LINKS_CLOSE
            && let Some(start) = open.take()
        {
            found = Some((start, at + content.len()));
        }
        at += line.len();
    }
    found.map(|(start, end)| (&text[..start], &text[end..]))
}

/// Reads a resolved repo template. The read is bounded so an oversized file is
/// never loaded in full; `rel` is used in errors because that is the path a
/// client recognises.
fn read_repo_template(path: &Path, rel: &str) -> Result<String, Error> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| {
            file.take(MAX_REPO_TEMPLATE_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
        })
        .map_err(|e| Error::CannotRead {
            path: rel.to_string(),
            detail: e.to_string(),
        })?;
    if bytes.len() > MAX_REPO_TEMPLATE_BYTES {
        return Err(Error::CannotRead {
            path: rel.to_string(),
            detail: format!("template exceeds {MAX_REPO_TEMPLATE_BYTES} bytes"),
        });
    }
    String::from_utf8(bytes).map_err(|e| Error::CannotRead {
        path: rel.to_string(),
        detail: e.to_string(),
    })
}

/// The repo's own template: the first regular file named `pull_request_template.md`
/// (any case) in `.github/`, the repo root, then `docs/`. Returns its path
/// relative to the repo, written with `/`, and its text. A folder of templates
/// (`PULL_REQUEST_TEMPLATE/`) is not a file and is ignored. A folder that can't be
/// read counts as having none; a matching file that can't be read is an error.
///
/// A match whose resolved path is outside the resolved `repo_dir` (a symlink to
/// another place) is skipped, with a warning naming it: the body is published, so
/// a repo must not be able to make it carry a file from elsewhere. The resolved
/// path is also the one read, and a template over `MAX_REPO_TEMPLATE_BYTES` is an
/// error naming the repo-relative path and the limit.
pub fn find_repo_template(repo_dir: &Path) -> Result<Outcome<Option<(String, String)>>, Error> {
    let mut warnings = Vec::new();
    let root = std::fs::canonicalize(repo_dir).ok();
    for folder in [".github", "", "docs"] {
        let dir = repo_dir.join(folder);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut names: Vec<String> = entries
            .flatten()
            .filter(|e| std::fs::metadata(e.path()).is_ok_and(|m| m.is_file()))
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n.to_lowercase() == "pull_request_template.md")
            .collect();
        names.sort();
        for name in names {
            let rel = if folder.is_empty() {
                name.clone()
            } else {
                format!("{folder}/{name}")
            };
            let resolved = match (&root, std::fs::canonicalize(dir.join(&name))) {
                (Some(root), Ok(file)) if file.starts_with(root) => Some(file),
                _ => None,
            };
            let Some(file) = resolved else {
                warnings.push(Warning {
                    message: format!("pr template {rel} points outside the repo; ignored"),
                });
                continue;
            };
            let text = read_repo_template(&file, &rel)?;
            return Ok(Outcome {
                value: Some((rel, text)),
                warnings,
            });
        }
    }
    Ok(Outcome {
        value: None,
        warnings,
    })
}
