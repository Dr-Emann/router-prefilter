#![doc = include_str!("../README.md")]
#![warn(variant_size_differences)]
#![warn(unreachable_pub)]
#![deny(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(unnameable_types)]

mod inner_prefilter;
pub mod matchers;

use crate::matchers::{Matcher, MatcherVisitor};
use bstr::BString;
use inner_prefilter::InnerPrefilter;
use std::cmp::Ordering;
use std::collections::{BTreeSet, btree_set};
use std::iter::FusedIterator;

/// A prefilter for quickly identifying potentially matching route patterns.
///
/// The prefilter analyzes route matchers to extract literal prefixes and builds
/// an efficient data structure for fast lookup. Routes without extractable
/// prefixes are tracked separately as always-possible matches.
///
/// # Examples
///
/// ```
/// use router_prefilter::RouterPrefilter;
/// use router_prefilter::matchers::{Matcher, MatcherVisitor};
///
/// struct Route {
///     path: String,
/// }
///
/// impl Matcher for Route {
///     fn visit(&self, visitor: &mut MatcherVisitor) {
///         visitor.visit_match_starts_with(&self.path);
///     }
/// }
///
/// let routes = vec![
///     Route { path: "/api".to_string() },
///     Route { path: "/users".to_string() },
/// ];
///
/// let mut prefilter = RouterPrefilter::new();
/// for (i, route) in routes.into_iter().enumerate() {
///     prefilter.insert(i, route);
/// }
/// let matches: Vec<_> = prefilter.possible_matches("/api/posts").collect();
/// assert!(matches.contains(&&0));
/// ```
#[derive(Debug)]
pub struct RouterPrefilter<K> {
    // Only includes indexes after prefilter starts
    always_possible: BTreeSet<K>,
    prefilter: InnerPrefilter<K>,

    builder: PrefilterBuilder,
}

impl<K: Clone> Clone for RouterPrefilter<K> {
    fn clone(&self) -> Self {
        Self {
            always_possible: self.always_possible.clone(),
            prefilter: self.prefilter.clone(),

            builder: PrefilterBuilder::new(),
        }
    }
}

impl<K: Ord> Default for RouterPrefilter<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K> RouterPrefilter<K> {
    /// Creates a new empty prefilter.
    ///
    /// # Examples
    ///
    /// ```
    /// use router_prefilter::RouterPrefilter;
    ///
    /// let prefilter: RouterPrefilter<usize> = RouterPrefilter::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            always_possible: BTreeSet::new(),
            prefilter: InnerPrefilter::new(),

            builder: PrefilterBuilder::new(),
        }
    }

    /// Returns whether this prefilter can perform filtering.
    ///
    /// Returns `true` if at least one matcher has been inserted with extractable
    /// prefixes. Returns `false` if the prefilter is empty or all matchers lack
    /// extractable prefixes.
    ///
    /// # Examples
    ///
    /// ```
    /// use router_prefilter::RouterPrefilter;
    /// use router_prefilter::matchers::{Matcher, MatcherVisitor};
    ///
    /// struct Route(&'static str);
    ///
    /// impl Matcher for Route {
    ///     fn visit(&self, visitor: &mut MatcherVisitor) {
    ///         visitor.visit_match_starts_with(self.0);
    ///     }
    /// }
    ///
    /// let mut prefilter = RouterPrefilter::new();
    /// assert!(!prefilter.can_prefilter());
    ///
    /// prefilter.insert(0, Route("/api"));
    /// assert!(prefilter.can_prefilter());
    /// ```
    #[must_use]
    pub fn can_prefilter(&self) -> bool {
        !self.prefilter.is_empty()
    }

    /// Returns the number of routes with extractable prefixes.
    ///
    /// A "prefilterable" route is one from which literal prefixes can be
    /// extracted for fast filtering. Routes without extractable prefixes
    /// are tracked separately as always-possible matches and are not
    /// counted by this method.
    ///
    /// A pattern must be anchored at the start and begin with literal
    /// characters to have an extractable prefix.
    ///
    /// # Examples
    ///
    /// ```
    /// use router_prefilter::RouterPrefilter;
    /// use router_prefilter::matchers::{Matcher, MatcherVisitor};
    ///
    /// struct Route {
    ///     pattern: &'static str,
    /// }
    ///
    /// impl Matcher for Route {
    ///     fn visit(&self, visitor: &mut MatcherVisitor) {
    ///         visitor.visit_match_regex(self.pattern);
    ///     }
    /// }
    ///
    /// let mut prefilter = RouterPrefilter::new();
    ///
    /// // Anchored with literal prefix - prefilterable
    /// prefilter.insert(0, Route { pattern: r"^/api/.*" });
    /// prefilter.insert(1, Route { pattern: r"^/users/\d+$" });
    ///
    /// // Anchored but no literal prefix - not prefilterable
    /// prefilter.insert(2, Route { pattern: r"^.*abc" });
    /// prefilter.insert(3, Route { pattern: r"^\d+/api" });
    ///
    /// // Not anchored - not prefilterable
    /// prefilter.insert(4, Route { pattern: r"/abc/def" });
    ///
    /// // Only routes 0 and 1 have extractable literal prefixes
    /// assert_eq!(prefilter.prefilterable_routes(), 2);
    /// ```
    #[must_use]
    pub fn prefilterable_routes(&self) -> usize {
        self.prefilter.num_routes()
    }
}

impl<K: Ord> RouterPrefilter<K> {
    /// Returns the total number of routes in the prefilter.
    ///
    /// This includes both routes with extractable prefixes and routes
    /// tracked as always-possible matches.
    ///
    /// # Examples
    ///
    /// ```
    /// use router_prefilter::RouterPrefilter;
    /// use router_prefilter::matchers::{Matcher, MatcherVisitor};
    ///
    /// struct Route {
    ///     pattern: &'static str,
    /// }
    ///
    /// impl Matcher for Route {
    ///     fn visit(&self, visitor: &mut MatcherVisitor) {
    ///         visitor.visit_match_regex(self.pattern);
    ///     }
    /// }
    ///
    /// let mut prefilter = RouterPrefilter::new();
    /// prefilter.insert(0, Route { pattern: r"^/api/.*" });
    /// prefilter.insert(1, Route { pattern: r"^.*abc" });
    ///
    /// assert_eq!(prefilter.len(), 2);
    /// ```
    #[must_use]
    pub fn len(&self) -> usize {
        self.prefilter.num_routes() + self.always_possible.len()
    }

    /// Returns whether the prefilter contains any routes.
    ///
    /// # Examples
    ///
    /// ```
    /// use router_prefilter::RouterPrefilter;
    /// use router_prefilter::matchers::{Matcher, MatcherVisitor};
    ///
    /// struct Route {
    ///     pattern: &'static str,
    /// }
    ///
    /// impl Matcher for Route {
    ///     fn visit(&self, visitor: &mut MatcherVisitor) {
    ///         visitor.visit_match_regex(self.pattern);
    ///     }
    /// }
    ///
    /// let mut prefilter: RouterPrefilter<usize> = RouterPrefilter::new();
    /// assert!(prefilter.is_empty());
    ///
    /// prefilter.insert(0, Route { pattern: r"^/api/.*" });
    /// assert!(!prefilter.is_empty());
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.always_possible.is_empty() && self.prefilter.is_empty()
    }

    /// Inserts a matcher with the given key.
    ///
    /// The matcher is analyzed to extract literal prefixes for fast filtering.
    /// If no prefixes can be extracted, the matcher is tracked as always-possible.
    ///
    /// This is a wrapper around [`PrefilterBuilder::compute_prefilter`]
    /// and [`Self::insert_prefilter`]
    ///
    /// # Examples
    ///
    /// ```
    /// use router_prefilter::RouterPrefilter;
    /// use router_prefilter::matchers::{Matcher, MatcherVisitor};
    ///
    /// struct Route(&'static str);
    ///
    /// impl Matcher for Route {
    ///     fn visit(&self, visitor: &mut MatcherVisitor) {
    ///         visitor.visit_match_starts_with(self.0);
    ///     }
    /// }
    ///
    /// let mut prefilter = RouterPrefilter::new();
    /// prefilter.insert(0, Route("/api"));
    /// prefilter.insert(1, Route("/users"));
    /// ```
    pub fn insert<M: Matcher>(&mut self, key: K, matcher: M)
    where
        K: Clone,
    {
        let match_prefilter = self.builder.compute_prefilter(matcher);
        self.insert_prefilter(key, match_prefilter)
    }

    /// Insert an optional prefilter for a matcher with the given key.
    ///
    /// If `match_prefilter` is `None`, the specified key will always be returned from
    /// [`Self::possible_matches`], otherwise, the prefilter will be used to filter matches.
    ///
    /// # Examples
    ///
    /// ```
    /// use router_prefilter::{RouterPrefilter, PrefilterBuilder};
    /// use router_prefilter::matchers::{Matcher, MatcherVisitor};
    ///
    /// struct Route(&'static str);
    ///
    /// impl Matcher for Route {
    ///     fn visit(&self, visitor: &mut MatcherVisitor) {
    ///         visitor.visit_match_starts_with(self.0);
    ///     }
    /// }
    ///
    /// let mut builder = PrefilterBuilder::new();
    /// let mut prefilter = RouterPrefilter::new();
    ///
    /// // Insert with a computed prefilter
    /// let match_prefilter = builder.compute_prefilter(Route("/api"));
    /// prefilter.insert_prefilter(0, match_prefilter);
    ///
    /// // Insert as always-possible (no prefilter)
    /// prefilter.insert_prefilter(1, None);
    ///
    /// let matches: Vec<_> = prefilter.possible_matches("/api/v1").collect();
    /// assert!(matches.contains(&&0));
    /// assert!(matches.contains(&&1));
    ///
    /// let matches: Vec<_> = prefilter.possible_matches("/other").collect();
    /// assert!(!matches.contains(&&0));
    /// assert!(matches.contains(&&1));
    /// ```
    pub fn insert_prefilter(&mut self, key: K, match_prefilter: Option<MatchPrefilter>)
    where
        K: Clone,
    {
        if let Some(MatchPrefilter { prefixes }) = match_prefilter {
            // Clean up in case this key was previously in always_possible
            self.always_possible.remove(&key);
            self.prefilter.insert(key, prefixes);
        } else {
            // Clean up in case this key was previously in the prefilter
            self.prefilter.remove(&key);
            self.always_possible.insert(key);
        }
    }

    /// Removes a matcher by key.
    ///
    /// # Examples
    ///
    /// ```
    /// use router_prefilter::RouterPrefilter;
    /// use router_prefilter::matchers::{Matcher, MatcherVisitor};
    ///
    /// struct Route(&'static str);
    ///
    /// impl Matcher for Route {
    ///     fn visit(&self, visitor: &mut MatcherVisitor) {
    ///         visitor.visit_match_starts_with(self.0);
    ///     }
    /// }
    ///
    /// let mut prefilter = RouterPrefilter::new();
    /// prefilter.insert(0, Route("/api"));
    /// prefilter.remove(&0);
    /// ```
    pub fn remove(&mut self, key: &K) {
        self.always_possible.remove(key);
        self.prefilter.remove(key);
    }

    /// Removes all routes from the prefilter.
    ///
    /// # Examples
    ///
    /// ```
    /// use router_prefilter::RouterPrefilter;
    /// use router_prefilter::matchers::{Matcher, MatcherVisitor};
    ///
    /// struct Route(&'static str);
    ///
    /// impl Matcher for Route {
    ///     fn visit(&self, visitor: &mut MatcherVisitor) {
    ///         visitor.visit_match_starts_with(self.0);
    ///     }
    /// }
    ///
    /// let mut prefilter = RouterPrefilter::new();
    /// prefilter.insert(0, Route("/api"));
    /// prefilter.insert(1, Route("/users"));
    ///
    /// assert_eq!(prefilter.len(), 2);
    /// prefilter.clear();
    /// assert!(prefilter.is_empty());
    /// ```
    pub fn clear(&mut self) {
        self.always_possible.clear();
        self.prefilter.clear();
    }

    /// Returns an iterator over matcher indexes that may match the given value.
    ///
    /// # Examples
    ///
    /// ```
    /// use router_prefilter::RouterPrefilter;
    /// use router_prefilter::matchers::{Matcher, MatcherVisitor};
    ///
    /// struct Route(&'static str);
    ///
    /// impl Matcher for Route {
    ///     fn visit(&self, visitor: &mut MatcherVisitor) {
    ///         visitor.visit_match_starts_with(self.0);
    ///     }
    /// }
    ///
    /// let routes = vec![Route("/api"), Route("/users")];
    /// let mut prefilter = RouterPrefilter::new();
    /// for (i, route) in routes.into_iter().enumerate() {
    ///     prefilter.insert(i, route);
    /// }
    ///
    /// let matches: Vec<_> = prefilter.possible_matches("/api/v1").collect();
    /// assert_eq!(matches, vec![&0]);
    /// ```
    #[must_use]
    #[doc(alias = "iter")]
    pub fn possible_matches<'a>(&'a self, value: &'a str) -> RouterPrefilterIter<'a, K> {
        let value = value.as_bytes();
        let filtered_keys = self.prefilter.check(value);
        let inner = if filtered_keys.is_empty() {
            RouterPrefilterIterState::OnlyAlways(self.always_possible.iter())
        } else {
            RouterPrefilterIterState::Union(UnionIter::new(
                self.always_possible.iter(),
                filtered_keys.into_iter(),
            ))
        };
        RouterPrefilterIter(inner)
    }

    /// Returns a mutable reference to the embedded prefilter builder.
    ///
    /// This can be more efficient than creating a new [`PrefilterBuilder`] every time,
    /// as it reuses the internal allocations.
    ///
    /// # Examples
    ///
    /// ```
    /// use router_prefilter::RouterPrefilter;
    /// use router_prefilter::matchers::{Matcher, MatcherVisitor};
    ///
    /// struct Route(&'static str);
    ///
    /// impl Matcher for Route {
    ///     fn visit(&self, visitor: &mut MatcherVisitor) {
    ///         visitor.visit_match_starts_with(self.0);
    ///     }
    /// }
    ///
    /// let mut prefilter: RouterPrefilter<usize> = RouterPrefilter::new();
    /// let match_prefilter = prefilter.prefilter_builder().compute_prefilter(Route("/api"));
    /// prefilter.insert_prefilter(0, match_prefilter);
    /// ```
    #[must_use]
    pub fn prefilter_builder(&mut self) -> &mut PrefilterBuilder {
        &mut self.builder
    }
}

/// A builder for prefilters.
///
/// Use when either:
/// - Simply extracting the possible prefilters from individual matchers, without building a
///   [router prefilter][RouterPrefilter] for many possible matchers
/// - When more control, additional logging, etc is required than simply calling
///   [`RouterPrefilter::insert`]
///
/// # Examples
///
/// ```
/// use router_prefilter::PrefilterBuilder;
/// use router_prefilter::matchers::{Matcher, MatcherVisitor};
///
/// struct Route(&'static str);
///
/// impl Matcher for Route {
///     fn visit(&self, visitor: &mut MatcherVisitor) {
///         visitor.visit_match_starts_with(self.0);
///     }
/// }
///
/// let mut builder = PrefilterBuilder::new();
/// let prefilter = builder.compute_prefilter(Route("/api"));
/// assert!(prefilter.is_some());
///
/// let no_prefilter = builder.compute_prefilter(Route(""));
/// assert!(no_prefilter.is_none());
/// ```
#[derive(Debug)]
pub struct PrefilterBuilder {
    matcher_visitor: MatcherVisitor,
}

impl Default for PrefilterBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PrefilterBuilder {
    /// Create a new prefilter builder
    #[must_use]
    pub fn new() -> Self {
        Self {
            matcher_visitor: MatcherVisitor::new(),
        }
    }

    /// Compute a prefilter (if possible) for the given matcher.
    ///
    /// Returns `None` if no literal prefixes can be extracted from the matcher.
    ///
    /// # Panics
    ///
    /// Panics if the matcher's [`Matcher::visit`] implementation leaves unbalanced
    /// nesting (more calls to [`MatcherVisitor::visit_nested_start`] than
    /// [`MatcherVisitor::visit_nested_finish`], or vice versa).
    ///
    /// ```should_panic
    /// use router_prefilter::PrefilterBuilder;
    /// use router_prefilter::matchers::{Matcher, MatcherVisitor};
    ///
    /// struct UnbalancedMatcher;
    ///
    /// impl Matcher for UnbalancedMatcher {
    ///     fn visit(&self, visitor: &mut MatcherVisitor) {
    ///         visitor.visit_nested_start();
    ///         visitor.visit_match_starts_with("/api");
    ///         // missing visit_nested_finish
    ///     }
    /// }
    ///
    /// let mut builder = PrefilterBuilder::new();
    /// let _ = builder.compute_prefilter(UnbalancedMatcher); // panics
    /// ```
    ///
    /// # Examples
    ///
    /// ```
    /// use router_prefilter::PrefilterBuilder;
    /// use router_prefilter::matchers::{Matcher, MatcherVisitor};
    ///
    /// struct Route(&'static str);
    ///
    /// impl Matcher for Route {
    ///     fn visit(&self, visitor: &mut MatcherVisitor) {
    ///         visitor.visit_match_starts_with(self.0);
    ///     }
    /// }
    ///
    /// let mut builder = PrefilterBuilder::new();
    ///
    /// let prefilter = builder.compute_prefilter(Route("/api")).unwrap();
    /// let prefixes: Vec<_> = prefilter.prefixes().collect();
    /// assert_eq!(prefixes, vec![b"/api".as_slice()]);
    /// ```
    #[must_use]
    pub fn compute_prefilter<M: Matcher>(&mut self, matcher: M) -> Option<MatchPrefilter> {
        matcher.visit(&mut self.matcher_visitor);
        self.matcher_visitor
            .finish()
            .map(|prefixes| MatchPrefilter {
                prefixes: prefixes.into_iter().collect(),
            })
    }
}

/// The prefilter for a single matcher.
///
/// A prefilter for a match consists of a collection of prefixes, at least one of which must
/// match at the start of a string if the specified matcher would match.
///
/// Obtained via [`PrefilterBuilder::compute_prefilter`]. There is no public constructor;
/// the prefilter can only be produced by analyzing a [`Matcher`].
///
/// See [`RouterPrefilter::insert_prefilter`] for inserting one into a router prefilter.
///
/// This type exists for observing the prefiltering process: e.g. debug logging of required
/// prefixes before insertion.
///
/// # Examples
///
/// ```
/// use router_prefilter::PrefilterBuilder;
/// use router_prefilter::matchers::{Matcher, MatcherVisitor};
///
/// struct Route(&'static str);
///
/// impl Matcher for Route {
///     fn visit(&self, visitor: &mut MatcherVisitor) {
///         visitor.visit_match_starts_with(self.0);
///     }
/// }
///
/// let mut builder = PrefilterBuilder::new();
/// let prefilter = builder.compute_prefilter(Route("/api")).unwrap();
/// assert_eq!(prefilter.len(), 1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchPrefilter {
    prefixes: Vec<BString>,
}

impl MatchPrefilter {
    /// Returns the number of prefixes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.prefixes.len()
    }

    /// Returns `true` if there are no prefixes.
    ///
    /// Note: a `MatchPrefilter` produced by [`PrefilterBuilder::compute_prefilter`] always
    /// contains at least one prefix, so this will always return `false` for such values.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.prefixes.is_empty()
    }

    /// Iterates over the prefixes for this prefilter.
    ///
    /// At least one of these prefixes must appear at the start of a string for the
    /// corresponding matcher to possibly match.
    ///
    /// This is currently the only method of prefiltering, but callers should not rely on this:
    /// this method is intended mainly for internal observability into the prefiltering process.
    ///
    /// # Examples
    ///
    /// ```
    /// use router_prefilter::PrefilterBuilder;
    /// use router_prefilter::matchers::{Matcher, MatcherVisitor};
    ///
    /// struct Route(&'static str);
    ///
    /// impl Matcher for Route {
    ///     fn visit(&self, visitor: &mut MatcherVisitor) {
    ///         visitor.visit_match_starts_with(self.0);
    ///     }
    /// }
    ///
    /// let mut builder = PrefilterBuilder::new();
    /// let prefilter = builder.compute_prefilter(Route("/api")).unwrap();
    /// let prefixes: Vec<_> = prefilter.prefixes().collect();
    /// assert_eq!(prefixes, vec![b"/api".as_slice()]);
    /// ```
    pub fn prefixes(&self) -> impl Iterator<Item = &[u8]> {
        self.prefixes.iter().map(|prefix| prefix.as_slice())
    }
}

/// Iterator over matcher indexes that may match a given value.
///
/// Created by [`RouterPrefilter::possible_matches`]. Yields matcher indexes
/// in ascending order.
pub struct RouterPrefilterIter<'a, K>(RouterPrefilterIterState<'a, K>);

enum RouterPrefilterIterState<'a, K> {
    OnlyAlways(btree_set::Iter<'a, K>),
    Union(UnionIter<'a, K>),
}

impl<'a, K: Ord> Iterator for RouterPrefilterIter<'a, K> {
    type Item = &'a K;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.0 {
            RouterPrefilterIterState::OnlyAlways(inner) => inner.next(),
            RouterPrefilterIterState::Union(inner) => inner.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match &self.0 {
            RouterPrefilterIterState::OnlyAlways(inner) => inner.size_hint(),
            RouterPrefilterIterState::Union(inner) => inner.size_hint(),
        }
    }

    fn fold<B, F>(self, init: B, f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        match self.0 {
            RouterPrefilterIterState::OnlyAlways(inner) => inner.fold(init, f),
            RouterPrefilterIterState::Union(inner) => inner.fold(init, f),
        }
    }
}

impl<K: Ord> ExactSizeIterator for RouterPrefilterIter<'_, K> {}

impl<K: Ord> FusedIterator for RouterPrefilterIter<'_, K> {}

impl<K: std::fmt::Debug> std::fmt::Debug for RouterPrefilterIter<'_, K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            RouterPrefilterIterState::OnlyAlways(inner) => {
                f.debug_tuple("RouterPrefilterIter").field(inner).finish()
            }
            RouterPrefilterIterState::Union(inner) => {
                f.debug_tuple("RouterPrefilterIter").field(inner).finish()
            }
        }
    }
}

// Iterator over the union of always and filtered keys
//
// We require that a key will not be in both `always` and `filtered` sets
#[derive(Debug)]
struct UnionIter<'a, K> {
    always: btree_set::Iter<'a, K>,
    filtered: btree_set::IntoIter<&'a K>,
    peeked: Option<Peeked<'a, K>>,
}

#[derive(Debug)]
enum Peeked<'a, K> {
    Always(&'a K),
    Filtered(&'a K),
}

impl<'a, K> UnionIter<'a, K> {
    fn new(always: btree_set::Iter<'a, K>, filtered: btree_set::IntoIter<&'a K>) -> Self {
        Self {
            always,
            filtered,
            peeked: None,
        }
    }
}

impl<'a, K: Ord> Iterator for UnionIter<'a, K> {
    type Item = &'a K;

    fn next(&mut self) -> Option<Self::Item> {
        let always_next;
        let filtered_next;
        match self.peeked.take() {
            Some(Peeked::Always(next)) => {
                always_next = Some(next);
                filtered_next = self.filtered.next();
            }
            Some(Peeked::Filtered(next)) => {
                always_next = self.always.next();
                filtered_next = Some(next);
            }
            None => {
                always_next = self.always.next();
                filtered_next = self.filtered.next();
            }
        }
        match (always_next, filtered_next) {
            (Some(a), Some(f)) => {
                let (returned, next_peeked) = match a.cmp(&f) {
                    Ordering::Less => (a, Peeked::Filtered(f)),
                    Ordering::Greater => (f, Peeked::Always(a)),
                    Ordering::Equal => {
                        unreachable!("keys cannot be both always found and filtered")
                    }
                };
                self.peeked = Some(next_peeked);
                Some(returned)
            }
            (Some(k), None) | (None, Some(k)) => Some(k),
            (None, None) => None,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // We require non-overlapping values
        let len = self.always.len() + self.filtered.len() + usize::from(self.peeked.is_some());
        (len, Some(len))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct TestMatcher {
        prefix: Option<&'static str>,
    }

    impl TestMatcher {
        fn with_prefix(prefix: &'static str) -> Self {
            Self {
                prefix: Some(prefix),
            }
        }

        fn without_prefix() -> Self {
            Self { prefix: None }
        }
    }

    impl Matcher for TestMatcher {
        fn visit(&self, visitor: &mut MatcherVisitor) {
            if let Some(prefix) = self.prefix {
                visitor.visit_match_starts_with(prefix);
            }
        }
    }

    #[test]
    fn test_iterator_no_skips_before_prefilter() {
        let matchers = vec![
            TestMatcher::without_prefix(),
            TestMatcher::without_prefix(),
            TestMatcher::without_prefix(),
            TestMatcher::without_prefix(),
            TestMatcher::with_prefix("/api"),
            TestMatcher::with_prefix("/users"),
        ];

        let mut prefilter = RouterPrefilter::new();
        for (i, matcher) in matchers.into_iter().enumerate() {
            prefilter.insert(i, matcher);
        }
        let matches: Vec<_> = prefilter.possible_matches("/api/test").collect();

        assert_eq!(matches, vec![&0, &1, &2, &3, &4]);
    }

    #[test]
    fn test_mixed_matchers() {
        let matchers = vec![
            TestMatcher::without_prefix(),
            TestMatcher::without_prefix(),
            TestMatcher::without_prefix(),
            TestMatcher::with_prefix("/api"),
        ];

        let mut prefilter = RouterPrefilter::new();
        for (i, matcher) in matchers.into_iter().enumerate() {
            prefilter.insert(i, matcher);
        }

        let matches: Vec<_> = prefilter.possible_matches("/api/test").collect();
        assert_eq!(matches, vec![&0, &1, &2, &3]);

        let matches: Vec<_> = prefilter.possible_matches("/other/path").collect();
        assert_eq!(matches, vec![&0, &1, &2]);
    }

    #[test]
    fn test_clone() {
        let mut prefilter = RouterPrefilter::new();
        prefilter.insert(0, TestMatcher::with_prefix("/api"));
        prefilter.insert(1, TestMatcher::without_prefix());

        let cloned = prefilter.clone();
        let matches: Vec<_> = cloned.possible_matches("/api/test").collect();
        assert_eq!(matches, vec![&0, &1]);
    }

    #[test]
    fn test_default() {
        let prefilter: RouterPrefilter<usize> = RouterPrefilter::default();
        assert!(prefilter.is_empty());
        assert!(!prefilter.can_prefilter());
    }

    #[test]
    fn test_utility_methods() {
        let mut prefilter = RouterPrefilter::new();

        // Empty state
        assert!(prefilter.is_empty());
        assert_eq!(prefilter.len(), 0);
        assert!(!prefilter.can_prefilter());
        assert_eq!(prefilter.prefilterable_routes(), 0);

        // Add prefilterable route
        prefilter.insert(0, TestMatcher::with_prefix("/api"));
        assert!(!prefilter.is_empty());
        assert_eq!(prefilter.len(), 1);
        assert!(prefilter.can_prefilter());
        assert_eq!(prefilter.prefilterable_routes(), 1);

        // Add non-prefilterable route
        prefilter.insert(1, TestMatcher::without_prefix());
        assert_eq!(prefilter.len(), 2);
        assert_eq!(prefilter.prefilterable_routes(), 1); // Still only 1 prefilterable

        // Add another prefilterable route
        prefilter.insert(2, TestMatcher::with_prefix("/users"));
        assert_eq!(prefilter.len(), 3);
        assert_eq!(prefilter.prefilterable_routes(), 2);
    }

    #[test]
    fn test_remove() {
        let mut prefilter = RouterPrefilter::new();
        prefilter.insert(0, TestMatcher::with_prefix("/api"));
        prefilter.insert(1, TestMatcher::without_prefix());
        prefilter.insert(2, TestMatcher::with_prefix("/users"));

        assert_eq!(prefilter.len(), 3);

        // Remove prefilterable route
        prefilter.remove(&0);
        assert_eq!(prefilter.len(), 2);
        let matches: Vec<_> = prefilter.possible_matches("/api/test").collect();
        assert!(!matches.contains(&&0));
        assert!(matches.contains(&&1));

        // Remove non-prefilterable route
        prefilter.remove(&1);
        assert_eq!(prefilter.len(), 1);
        let matches: Vec<_> = prefilter.possible_matches("/users/test").collect();
        assert!(!matches.contains(&&1));
        assert!(matches.contains(&&2));

        // Remove last route
        prefilter.remove(&2);
        assert!(prefilter.is_empty());
    }

    #[test]
    fn test_iterator_fold() {
        let mut prefilter = RouterPrefilter::new();
        prefilter.insert(0, TestMatcher::with_prefix("/api"));
        prefilter.insert(1, TestMatcher::with_prefix("/users"));

        let sum = prefilter.possible_matches("/api/test").sum::<i32>();
        assert_eq!(sum, 0); // Only route 0 matches

        let sum = prefilter.possible_matches("/users/test").sum::<i32>();
        assert_eq!(sum, 1); // Only route 1 matches
    }

    #[test]
    fn test_iterator_size_hint() {
        let mut prefilter = RouterPrefilter::new();
        prefilter.insert(0, TestMatcher::with_prefix("/api"));
        prefilter.insert(1, TestMatcher::without_prefix());

        let iter = prefilter.possible_matches("/api/test");
        let (min, max) = iter.size_hint();
        assert!(min <= max.unwrap_or(usize::MAX));
    }

    #[test]
    fn test_iterator_exact_size() {
        let mut prefilter = RouterPrefilter::new();
        prefilter.insert(0, TestMatcher::with_prefix("/api"));
        prefilter.insert(1, TestMatcher::without_prefix());
        prefilter.insert(2, TestMatcher::with_prefix("/users"));

        // Union case: prefilter result + always_possible
        let iter = prefilter.possible_matches("/api/test");
        assert_eq!(iter.len(), 2); // routes 0 and 1
        let (min, max) = iter.size_hint();
        assert_eq!(min, 2);
        assert_eq!(max, Some(2));

        // OnlyAlways case: no prefilter matches
        let iter = prefilter.possible_matches("/other/path");
        assert_eq!(iter.len(), 1); // only route 1
        let (min, max) = iter.size_hint();
        assert_eq!(min, 1);
        assert_eq!(max, Some(1));

        // Union case: size_hint must stay accurate after consuming elements
        let mut iter = prefilter.possible_matches("/api/test");
        assert_eq!(iter.len(), 2);
        iter.next();
        assert_eq!(iter.len(), 1);
        iter.next();
        assert_eq!(iter.len(), 0);
    }

    #[test]
    fn test_iterator_debug() {
        let mut prefilter = RouterPrefilter::new();
        prefilter.insert("key 123", TestMatcher::with_prefix("/api"));

        let iter = prefilter.possible_matches("/api/test");
        let debug_str = format!("{:?}", iter);
        assert!(debug_str.contains("RouterPrefilterIter"));
        assert!(debug_str.contains("key 123"));
    }

    #[test]
    fn test_iterator_fused() {
        let mut prefilter = RouterPrefilter::new();
        prefilter.insert(0, TestMatcher::with_prefix("/api"));

        let mut iter = prefilter.possible_matches("/api/test");

        // Exhaust the iterator
        assert_eq!(iter.next(), Some(&0));
        assert_eq!(iter.next(), None);

        // FusedIterator guarantees None forever after
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_duplicate_key_insert_replaces_prefix() {
        let mut prefilter = RouterPrefilter::new();
        prefilter.insert(0, TestMatcher::with_prefix("/api"));
        prefilter.insert(0, TestMatcher::with_prefix("/users"));

        assert_eq!(prefilter.len(), 1);
        assert_eq!(prefilter.prefilterable_routes(), 1);

        // Old prefix should no longer match
        let matches: Vec<_> = prefilter.possible_matches("/api/test").collect();
        assert!(!matches.contains(&&0));

        // New prefix should match
        let matches: Vec<_> = prefilter.possible_matches("/users/test").collect();
        assert!(matches.contains(&&0));
    }

    #[test]
    fn test_duplicate_key_insert_prefilterable_to_always() {
        let mut prefilter = RouterPrefilter::new();
        prefilter.insert(0, TestMatcher::with_prefix("/api"));
        prefilter.insert(0, TestMatcher::without_prefix());

        assert_eq!(prefilter.len(), 1);
        assert_eq!(prefilter.prefilterable_routes(), 0);

        // Should now be in always_possible, matching everything
        let matches: Vec<_> = prefilter.possible_matches("/anything").collect();
        assert!(matches.contains(&&0));
    }

    #[test]
    fn test_duplicate_key_insert_always_to_prefilterable() {
        let mut prefilter = RouterPrefilter::new();
        prefilter.insert(0, TestMatcher::without_prefix());
        prefilter.insert(0, TestMatcher::with_prefix("/api"));

        assert_eq!(prefilter.len(), 1);
        assert_eq!(prefilter.prefilterable_routes(), 1);

        // Should only match the new prefix
        let matches: Vec<_> = prefilter.possible_matches("/api/test").collect();
        assert!(matches.contains(&&0));

        let matches: Vec<_> = prefilter.possible_matches("/other").collect();
        assert!(!matches.contains(&&0));
    }

    #[test]
    fn test_duplicate_key_insert_then_remove() {
        let mut prefilter = RouterPrefilter::new();
        prefilter.insert(0, TestMatcher::with_prefix("/api"));
        prefilter.insert(0, TestMatcher::with_prefix("/users"));
        prefilter.remove(&0);

        assert!(prefilter.is_empty());
        assert_eq!(prefilter.len(), 0);

        // Nothing should match after removal
        let matches: Vec<_> = prefilter.possible_matches("/api/test").collect();
        assert!(matches.is_empty());
        let matches: Vec<_> = prefilter.possible_matches("/users/test").collect();
        assert!(matches.is_empty());
    }

    #[test]
    fn test_nested_prefix_chain() {
        // Each prefix is a prefix of the ones above it, inserted longest-first
        let matchers = vec![
            TestMatcher::with_prefix("/a/a/a/a/a/a/a/a/a/a"),
            TestMatcher::with_prefix("/a/a/a/a/a/a/a/a/a"),
            TestMatcher::with_prefix("/a/a/a/a/a/a/a/a"),
            TestMatcher::with_prefix("/a/a/a/a/a/a/a"),
            TestMatcher::with_prefix("/a/a/a/a/a/a"),
            TestMatcher::with_prefix("/a/a/a/a/a"),
            TestMatcher::with_prefix("/a/a/a/a"),
            TestMatcher::with_prefix("/a/a/a"),
            TestMatcher::with_prefix("/a/a"),
            TestMatcher::with_prefix("/a"),
            TestMatcher::with_prefix(""),
        ];

        let mut prefilter = RouterPrefilter::new();
        for (i, matcher) in matchers.into_iter().enumerate() {
            prefilter.insert(i, matcher);
        }

        // Full path matches all prefixes
        let matches: Vec<_> = prefilter
            .possible_matches("/a/a/a/a/a/a/a/a/a/a/end")
            .collect();
        assert_eq!(matches, vec![&0, &1, &2, &3, &4, &5, &6, &7, &8, &9, &10]);

        // Partial path matches only shorter prefixes
        let matches: Vec<_> = prefilter.possible_matches("/a/a/a/a/a/z").collect();
        assert_eq!(matches, vec![&5, &6, &7, &8, &9, &10]);

        // Shortest non-empty prefix
        let matches: Vec<_> = prefilter.possible_matches("/a/z").collect();
        assert_eq!(matches, vec![&9, &10]);

        // No non-empty prefix matches, but empty prefix is always possible
        let matches: Vec<_> = prefilter.possible_matches("/b").collect();
        assert_eq!(matches, vec![&10]);

        // Empty string matches empty prefix (always possible)
        let matches: Vec<_> = prefilter.possible_matches("").collect();
        assert_eq!(matches, vec![&10]);

        // Empty prefix goes into always_possible, not the prefilter
        assert_eq!(prefilter.prefilterable_routes(), 10);
    }

    #[test]
    fn test_prefilter_builder_standalone() {
        let mut builder = PrefilterBuilder::new();

        let prefilter = builder.compute_prefilter(TestMatcher::with_prefix("/api")).unwrap();
        let prefixes: Vec<_> = prefilter.prefixes().collect();
        assert_eq!(prefixes, vec![b"/api".as_slice()]);

        let none = builder.compute_prefilter(TestMatcher::without_prefix());
        assert!(none.is_none());
    }

    #[test]
    fn test_prefilter_builder_reuse() {
        let mut builder = PrefilterBuilder::new();

        let p1 = builder.compute_prefilter(TestMatcher::with_prefix("/api")).unwrap();
        let p2 = builder.compute_prefilter(TestMatcher::with_prefix("/api")).unwrap();
        assert_eq!(p1, p2);

        let p3 = builder.compute_prefilter(TestMatcher::with_prefix("/users")).unwrap();
        assert_ne!(p1, p3);
    }

    #[test]
    fn test_insert_prefilter_with_prefilter() {
        let mut builder = PrefilterBuilder::new();
        let match_prefilter = builder.compute_prefilter(TestMatcher::with_prefix("/api"));

        let mut prefilter = RouterPrefilter::new();
        prefilter.insert_prefilter(0, match_prefilter);

        let matches: Vec<_> = prefilter.possible_matches("/api/test").collect();
        assert!(matches.contains(&&0));

        let matches: Vec<_> = prefilter.possible_matches("/other").collect();
        assert!(!matches.contains(&&0));
    }

    #[test]
    fn test_insert_prefilter_none_is_always_possible() {
        let mut prefilter = RouterPrefilter::new();
        prefilter.insert_prefilter(0, None);

        let matches: Vec<_> = prefilter.possible_matches("/anything").collect();
        assert!(matches.contains(&&0));

        assert_eq!(prefilter.prefilterable_routes(), 0);
        assert_eq!(prefilter.len(), 1);
    }

    #[test]
    fn test_prefilter_builder_accessor() {
        let mut prefilter: RouterPrefilter<usize> = RouterPrefilter::new();
        let match_prefilter = prefilter.prefilter_builder().compute_prefilter(TestMatcher::with_prefix("/api"));
        prefilter.insert_prefilter(0, match_prefilter);

        let matches: Vec<_> = prefilter.possible_matches("/api/test").collect();
        assert!(matches.contains(&&0));
    }

    #[test]
    fn test_match_prefilter_len_and_is_empty() {
        let mut builder = PrefilterBuilder::new();
        let prefilter = builder.compute_prefilter(TestMatcher::with_prefix("/api")).unwrap();
        assert_eq!(prefilter.len(), 1);
        assert!(!prefilter.is_empty());
    }

    #[test]
    fn test_match_prefilter_clone_and_eq() {
        let mut builder = PrefilterBuilder::new();
        let prefilter = builder.compute_prefilter(TestMatcher::with_prefix("/api")).unwrap();
        let cloned = prefilter.clone();
        assert_eq!(prefilter, cloned);
    }

    #[test]
    fn test_match_prefilter_insert_twice_from_clone() {
        let mut builder = PrefilterBuilder::new();
        let match_prefilter = builder.compute_prefilter(TestMatcher::with_prefix("/api")).unwrap();

        let mut prefilter = RouterPrefilter::new();
        prefilter.insert_prefilter(0, Some(match_prefilter.clone()));
        prefilter.insert_prefilter(1, Some(match_prefilter));

        let matches: Vec<_> = prefilter.possible_matches("/api/test").collect();
        assert!(matches.contains(&&0));
        assert!(matches.contains(&&1));
    }

    #[test]
    #[should_panic = "mismatched nesting calls to MatcherVisitor"]
    fn test_compute_prefilter_panics_on_unbalanced_nesting() {
        struct BadMatcher;

        impl Matcher for BadMatcher {
            fn visit(&self, visitor: &mut MatcherVisitor) {
                visitor.visit_nested_start();
                visitor.visit_match_starts_with("/api");
            }
        }

        let mut builder = PrefilterBuilder::new();
        let _ = builder.compute_prefilter(BadMatcher);
    }
}
