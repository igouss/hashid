//! The plain-C structs a caller fills in, and the borrows that validate them.
//!
//! Every field crosses as a C primitive or a borrowed `const char *`. Nothing
//! here owns anything: a struct is read during the call that receives it and
//! never retained, so the caller's strings need only outlive that call.

use std::ffi::{c_char, CStr};

use hashid::{IdConfig, Prefix, Request};

use crate::status::{fail, fail_with, hashid_status};

/// The content an ID is derived from.
///
/// Strings are borrowed for the duration of the call. `description` and
/// `creator` may be `NULL`, which hashes as the empty string.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct hashid_request {
    /// The primary content field. Always hashed. Required.
    pub title: *const c_char,
    /// Optional body. `NULL` for absent.
    pub description: *const c_char,
    /// Optional author. `NULL` for absent.
    pub creator: *const c_char,
    /// Creation instant, in nanoseconds since the Unix epoch, UTC.
    ///
    /// Not a platform date type, because this number is hashed verbatim: two
    /// implementations that round it differently mint different IDs for the
    /// same record. `hashid_now_nanos` produces one.
    pub created_at_nanos: i64,
    /// Roughly how many IDs already exist. Picks the starting hash length.
    ///
    /// Zero is safe — every candidate is still checked, and the ladder widens
    /// the hash as it collides — it only costs extra lookups on a large
    /// dataset. An estimate is entirely adequate.
    pub population: u64,
}

/// How IDs should look: which namespace, and how much entropy to spend.
///
/// `prefix` is taken **strictly** — it must already be a legal prefix. Run
/// human input through `hashid_normalize_prefix` first, because nothing here
/// renames your namespace silently.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct hashid_config {
    /// The namespace every minted ID carries. Taken strictly.
    pub prefix: *const c_char,
    /// Shortest hash the adaptive sizing may choose.
    pub min_hash_length: u32,
    /// Longest hash the adaptive sizing may choose.
    pub max_hash_length: u32,
    /// The birthday bound the sizing keeps below.
    pub max_collision_prob: f64,
}

/// A caller's `u64` as a `usize`, refusing what this platform cannot hold.
///
/// Nothing here clamps. A saturating conversion turns a caller's mistake into
/// a plausible number — a population of `u64::MAX` becomes "as many as this
/// machine can address" — and says nothing about it.
///
/// # Errors
///
/// `HASHID_ERR_OUT_OF_RANGE` when the value exceeds this platform's `usize`,
/// which can only happen on a 32-bit target.
pub(crate) fn narrow(value: u64, name: &str) -> Result<usize, hashid_status> {
    usize::try_from(value).map_err(|_| {
        fail(
            hashid_status::HASHID_ERR_OUT_OF_RANGE,
            format!("`{name}` is {value}, larger than this platform's usize"),
        )
    })
}

/// Borrow a C string.
///
/// # Safety
///
/// `pointer` must be NULL or a valid NUL-terminated string that outlives the
/// returned borrow.
pub(crate) unsafe fn borrow_str<'a>(
    pointer: *const c_char,
    name: &str,
) -> Result<&'a str, hashid_status> {
    if pointer.is_null() {
        return Err(fail(
            hashid_status::HASHID_ERR_NULL_ARGUMENT,
            format!("`{name}` must not be NULL"),
        ));
    }
    // SAFETY: delegated to this function's own contract.
    unsafe { CStr::from_ptr(pointer) }.to_str().map_err(|_| {
        fail(
            hashid_status::HASHID_ERR_INVALID_UTF8,
            format!("`{name}` is not valid UTF-8"),
        )
    })
}

/// Borrow a C string that is allowed to be absent.
///
/// # Safety
///
/// As [`borrow_str`].
pub(crate) unsafe fn borrow_optional_str<'a>(
    pointer: *const c_char,
    name: &str,
) -> Result<Option<&'a str>, hashid_status> {
    if pointer.is_null() {
        return Ok(None);
    }
    // SAFETY: delegated to this function's own contract.
    unsafe { borrow_str(pointer, name) }.map(Some)
}

/// Borrow a request struct as the core's request type.
///
/// # Safety
///
/// `request` must point at a valid `hashid_request` whose strings outlive the
/// returned borrow.
pub(crate) unsafe fn borrow_request<'a>(
    request: *const hashid_request,
) -> Result<Request<'a>, hashid_status> {
    if request.is_null() {
        return Err(fail(
            hashid_status::HASHID_ERR_NULL_ARGUMENT,
            "`request` must not be NULL",
        ));
    }
    // SAFETY: the caller contract is that `request` is valid.
    let request: &hashid_request = unsafe { &*request };

    // SAFETY: the caller contract covers every string in the struct.
    let title: &str = unsafe { borrow_str(request.title, "request.title") }?;
    // SAFETY: as above.
    let description: Option<&str> =
        unsafe { borrow_optional_str(request.description, "request.description") }?;
    // SAFETY: as above.
    let creator: Option<&str> = unsafe { borrow_optional_str(request.creator, "request.creator") }?;

    let mut core: Request<'a> = Request::new(title)
        // The instant crosses as the nanoseconds the core hashes, so there is
        // no date type to convert and nothing to reject.
        .at_nanos(request.created_at_nanos)
        .with_population(narrow(request.population, "request.population")?);
    if let Some(description) = description {
        core = core.with_description(description);
    }
    if let Some(creator) = creator {
        core = core.with_creator(creator);
    }
    Ok(core)
}

/// Validate a config struct into the core's config.
///
/// # Safety
///
/// `config` must point at a valid `hashid_config`.
pub(crate) unsafe fn borrow_config(
    config: *const hashid_config,
) -> Result<IdConfig, hashid_status> {
    if config.is_null() {
        return Err(fail(
            hashid_status::HASHID_ERR_NULL_ARGUMENT,
            "`config` must not be NULL",
        ));
    }
    // SAFETY: the caller contract is that `config` is valid.
    let config: &hashid_config = unsafe { &*config };
    // SAFETY: as above, for its one string.
    let prefix: &str = unsafe { borrow_str(config.prefix, "config.prefix") }?;

    let parsed: Prefix = Prefix::parse(prefix).map_err(|err| fail_with(&err))?;
    let min: usize = narrow(u64::from(config.min_hash_length), "config.min_hash_length")?;
    let max: usize = narrow(u64::from(config.max_hash_length), "config.max_hash_length")?;
    let probability: f64 = config.max_collision_prob;

    IdConfig::new(parsed)
        .with_hash_lengths(min, max)
        .and_then(|built: IdConfig| built.with_max_collision_prob(probability))
        .map_err(|err| fail_with(&err))
}
