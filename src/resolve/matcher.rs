//! Fragment matching over known IDs.

use crate::domain::id::Id;

/// Does `id`'s hash contain `fragment`?
///
/// If `fragment` carries a child path (`64up6.4`), the hash part matches as a
/// substring while the child path must match **exactly** — otherwise searching
/// for `64up6.4` would happily return the parent `br-64up6` and every sibling
/// of the child you asked for.
///
/// This is the single rule; [`find_matching`] is just this applied to a slice,
/// and an adapter over its own collection can apply it directly.
#[must_use]
pub fn matches_fragment(id: &Id, fragment: &str) -> bool {
    let (hash_fragment, wanted_child): (&str, Option<&str>) = match fragment.split_once('.') {
        Some((hash, child)) => (hash, Some(child)),
        None => (fragment, None),
    };

    if !id.hash().as_str().contains(hash_fragment) {
        return false;
    }

    match wanted_child {
        Some(child) => child_path_text(id).as_deref() == Some(child),
        None => true,
    }
}

/// Every ID in `known` matching `fragment`, per [`matches_fragment`].
#[must_use]
pub fn find_matching(known: &[Id], fragment: &str) -> Vec<Id> {
    known
        .iter()
        .filter(|id: &&Id| matches_fragment(id, fragment))
        .cloned()
        .collect()
}

/// An ID's child path rendered as `1.2`, or `None` for a root.
fn child_path_text(id: &Id) -> Option<String> {
    if id.is_root() {
        return None;
    }
    let segments: Vec<String> = id.child_path().iter().map(u32::to_string).collect();
    Some(segments.join("."))
}
