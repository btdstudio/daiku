//! The repo lock: one build at a time works on a repo from implement through ship
//! (spec R29). A port of Go's `internal/build/lock`.

use std::collections::BTreeMap;

use crate::state::machine::COMPLETE;
use crate::state::{BUILD_LOOP, Change};

/// The phases a build holds its repo in.
const HELD: [&str; 5] = ["implement", "test", "review", "security", "ship"];

/// Go's `Holds`: an open build in implement through ship, short of shipping.
pub fn holds(c: &Change) -> bool {
    if c.loop_ != BUILD_LOOP
        || c.is_closed()
        || c.repos.is_empty()
        || !HELD.contains(&c.phase.as_str())
    {
        return false;
    }
    !(c.phase == "ship" && c.status == COMPLETE)
}

/// Go's `Enters`: moving `c` to `phase` would take its repo.
pub fn enters(c: &Change, phase: &str) -> bool {
    c.loop_ == BUILD_LOOP && !c.repos.is_empty() && HELD.contains(&phase) && !holds(c)
}

/// Go's `Holder`: the first other build, by name, holding `c`'s repo.
pub fn holder<'a>(changes: &'a BTreeMap<String, Change>, c: &Change) -> Option<&'a Change> {
    if c.loop_ != BUILD_LOOP || c.repos.is_empty() {
        return None;
    }
    changes
        .values()
        .find(|o| o.name != c.name && holds(o) && o.repos[0] == c.repos[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(name: &str, repo: &str, phase: &str, status: &str) -> Change {
        Change {
            name: name.into(),
            loop_: BUILD_LOOP.into(),
            phase: phase.into(),
            status: status.into(),
            repos: vec![repo.into()],
            ..Change::default()
        }
    }

    #[test]
    fn holds_from_implement_until_shipped() {
        assert!(!holds(&build("S-1", "api", "tasks", "approved")));
        assert!(holds(&build("S-1", "api", "implement", "in-progress")));
        assert!(holds(&build("S-1", "api", "ship", "in-progress")));
        assert!(!holds(&build("S-1", "api", "ship", "complete")));
        let mut closed = build("S-1", "api", "review", "draft");
        closed.status = "closed".into();
        assert!(!holds(&closed));
        let mut plan = build("1-x", "api", "implement", "in-progress");
        plan.loop_ = "plan".into();
        assert!(!holds(&plan));
    }

    #[test]
    fn enters_and_holder() {
        let a = build("S-0001", "api", "implement", "in-progress");
        let b = build("S-0002", "api", "tasks", "approved");
        let c = build("S-0003", "web", "tasks", "approved");
        assert!(enters(&b, "implement") && !enters(&b, "tasks") && !enters(&a, "test"));
        let changes: BTreeMap<String, Change> = [a.clone(), b.clone(), c.clone()]
            .into_iter()
            .map(|c| (c.name.clone(), c))
            .collect();
        assert_eq!(
            holder(&changes, &b).map(|h| h.name.as_str()),
            Some("S-0001")
        );
        assert_eq!(holder(&changes, &a), None);
        assert_eq!(holder(&changes, &c), None);
        assert_eq!(
            crate::Error::RepoLocked {
                repo: "api".into(),
                holder: "S-0001".into(),
                phase: "implement".into(),
                status: "in-progress".into()
            }
            .to_string(),
            "daiku: api is locked by S-0001 (implement in-progress) — finish, close or reopen it to tasks"
        );
    }
}
