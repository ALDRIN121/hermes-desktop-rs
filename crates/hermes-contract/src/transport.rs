//! Port of `apps/desktop/electron/api-transport.ts` retry policy.
//!
//! The rule that matters: a transient transport error does not mean the server
//! did not process the request. Idempotent verbs may always retry; everything
//! else retries only when the request provably never reached the server.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    Idempotent,
    Mutating,
}

impl Verb {
    pub fn of(method: &str) -> Verb {
        match method.to_ascii_uppercase().as_str() {
            "GET" | "HEAD" | "OPTIONS" => Verb::Idempotent,
            _ => Verb::Mutating,
        }
    }
}

/// Errors that prove the request never reached the server.
pub fn never_sent(code: &str) -> bool {
    matches!(
        code,
        "ECONNREFUSED" | "ENOTFOUND" | "EAI_AGAIN" | "EHOSTUNREACH" | "ENETUNREACH"
    )
}

/// Errors worth retrying at all (subject to the verb gate).
pub fn transient(code: &str) -> bool {
    never_sent(code)
        || matches!(
            code,
            "ECONNRESET" | "EPIPE" | "ETIMEDOUT" | "socket hang up"
        )
}

/// `body_sent` is false when the failure happened before the request body
/// started flushing — the one ambiguous case that is still safe to replay.
pub fn should_retry(method: &str, code: &str, body_sent: bool) -> bool {
    if !transient(code) {
        return false;
    }
    match Verb::of(method) {
        Verb::Idempotent => true,
        Verb::Mutating => never_sent(code) || !body_sent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idempotent_verbs_retry_on_any_transient_error() {
        for code in ["ECONNRESET", "EPIPE", "ETIMEDOUT", "ECONNREFUSED"] {
            assert!(should_retry("GET", code, true), "GET {code}");
            assert!(should_retry("HEAD", code, true), "HEAD {code}");
            assert!(should_retry("OPTIONS", code, true), "OPTIONS {code}");
        }
    }

    #[test]
    fn mutating_verbs_retry_only_when_never_sent() {
        // Ambiguous: the body went out and the socket died. Never replay.
        assert!(!should_retry("POST", "ECONNRESET", true));
        assert!(!should_retry("PUT", "EPIPE", true));
        assert!(!should_retry("DELETE", "ETIMEDOUT", true));
        // Provably never reached the server.
        assert!(should_retry("POST", "ECONNREFUSED", true));
        assert!(should_retry("POST", "ENOTFOUND", false));
    }

    #[test]
    fn mutating_verbs_retry_when_body_not_yet_flushed() {
        assert!(should_retry("POST", "ECONNRESET", false));
    }

    #[test]
    fn non_transport_errors_never_retry() {
        assert!(!should_retry("GET", "EACCES", true));
        assert!(!should_retry("POST", "400", true));
    }
}
