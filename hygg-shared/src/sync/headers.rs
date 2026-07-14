//! HTTP header names for device authentication, shared by the hygg client and
//! server so both sides agree on the exact strings. Presenting a bearer token
//! is no longer sufficient on its own: an authenticated request must also carry
//! the account username and a stable per-machine id.

/// The full account username (email) presented alongside the bearer token. The
/// server rejects a token whose owner's email does not match this header, so a
/// leaked token is not a usable credential without also knowing the username.
pub const USER_HEADER: &str = "x-hygg-user";

/// A stable identifier for the client machine (like `/etc/machine-id`). A
/// device token binds to the first machine id it is seen with; a later request
/// presenting a different machine id is rejected, which blocks one token from
/// being copied to and used from multiple machines.
pub const MACHINE_ID_HEADER: &str = "x-hygg-machine-id";
