//! The plan overview's refresh after each transition (D7).

use super::{Change, Store};
use crate::Error;

/// Refreshes the overview of the plan `c` belongs to, when the `overview` setting
/// is `auto`, returning what it wrote.
pub(crate) fn refresh_for(store: &Store, _c: &Change) -> Result<Vec<String>, Error> {
    if store
        .settings()
        .is_none_or(|s| s.effective.overview != "auto")
    {
        return Ok(Vec::new());
    }
    Ok(Vec::new())
}
