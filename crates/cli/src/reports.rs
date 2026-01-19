use ariadne::{Cache, FnCache, Source};

/// A source cache for ariadne reports from `(id, source)` pairs.
pub fn sources<Id, S, I>(iter: I) -> impl Cache<Id>
where
    Id: std::fmt::Display + std::hash::Hash + PartialEq + Eq + Clone,
    I: IntoIterator<Item = (Id, S)>,
    S: AsRef<str>,
{
    FnCache::new((move |id| Err(format!("failed to fetch source '{id}'"))) as fn(&_) -> _)
        .with_sources(
            iter.into_iter()
                .map(|(id, s)| (id, Source::from(s)))
                .collect(),
        )
}
