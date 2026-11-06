//! Small numbers standing in for compositor window identities, which the paste
//! guard compares between recording start and paste.
use std::collections::HashMap;
use std::hash::Hash;

/// Windows remembered at once. Past this the oldest identity is forgotten.
const LIMIT: usize = 4096;

/// A token is never reused, even after its window is forgotten, so two different
/// windows can never look like the same paste target. A forgotten window that
/// returns gets a new token, which can only make the guard fail closed.
pub(super) struct FocusTokens<K> {
    tokens: HashMap<K, u32>,
    last: u32,
}

impl<K> Default for FocusTokens<K> {
    fn default() -> Self {
        Self {
            tokens: HashMap::new(),
            last: 0,
        }
    }
}

impl<K: Eq + Hash> FocusTokens<K> {
    /// `None` only once tokens run out, which fails closed.
    pub fn token(&mut self, key: K) -> Option<u32> {
        if let Some(token) = self.tokens.get(&key) {
            return Some(*token);
        }
        let token = self.last.checked_add(1)?;
        if self.tokens.len() >= LIMIT
            && let Some(oldest) = self.tokens.values().min().copied()
        {
            self.tokens.retain(|_, token| *token != oldest);
        }
        self.last = token;
        self.tokens.insert(key, token);
        Some(token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_session_keeps_resolving_and_never_reuses_a_token() {
        let mut tokens = FocusTokens::default();
        let first = tokens.token(0_u32).unwrap();
        let count = u32::try_from(LIMIT).unwrap() * 3;
        let mut seen = std::collections::HashSet::from([first]);
        for window in 1..=count {
            assert!(seen.insert(tokens.token(window).unwrap()), "token reused");
        }
        assert_eq!(tokens.tokens.len(), LIMIT, "memory stays bounded");
        assert_eq!(
            tokens.token(count),
            Some(count + 1),
            "a recent window is stable"
        );
        let returned = tokens.token(0).unwrap();
        assert!(
            !seen.contains(&returned),
            "a forgotten window never matches its old token"
        );
    }

    #[test]
    fn running_out_of_tokens_fails_closed() {
        let mut tokens = FocusTokens {
            tokens: HashMap::new(),
            last: u32::MAX,
        };
        assert_eq!(tokens.token(1_u32), None);
    }
}
