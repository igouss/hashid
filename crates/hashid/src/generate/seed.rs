//! Turning content into the exact bytes that get hashed.

use core::fmt::Write as _;

use crate::domain::request::Request;

/// Build the hash input for `request` at `nonce`.
///
/// Fields are **length-prefixed** — each is written as `<byte-len>:<value>` —
/// and that is the whole point. A naive `title|description|creator` join lets a
/// caller move the boundary: title `a|b` with no description would seed
/// identically to title `a` with description `b`, so two genuinely different
/// records could collide by construction rather than by chance. Prefixing each
/// field with its byte length makes the boundaries unforgeable.
///
/// The length is in **bytes**, not characters, so multi-byte content is
/// unambiguous too (`héllo wörld` is 13 bytes, not 11).
///
/// Field order is `title`, `description`, `creator`, `timestamp-nanos`,
/// `nonce`. Absent optional fields hash as the empty string, i.e. `0:`.
#[must_use]
pub fn seed(request: &Request<'_>, nonce: u32) -> String {
    let timestamp: String = request.created_at_nanos().to_string();
    let nonce: String = nonce.to_string();

    let mut out: String = String::new();
    push_field(&mut out, request.title);
    push_field(&mut out, request.description.unwrap_or(""));
    push_field(&mut out, request.creator.unwrap_or(""));
    push_field(&mut out, &timestamp);
    push_field(&mut out, &nonce);
    out
}

/// Append one `<byte-len>:<value>` field to `out`.
fn push_field(out: &mut String, value: &str) {
    // Writing to a String is infallible; the Result exists only to satisfy the
    // fmt::Write signature.
    let _ = write!(out, "{}:", value.len());
    out.push_str(value);
}
