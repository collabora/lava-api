use std::collections::HashSet;
use std::fmt::Display;
use std::hash::Hash;

/// Implement `QuerySetMember` to include a simple enum into a
/// QuerySet. This trait is necessary so that the query set can have
/// some way of knowing what the full set of values in the enum is.
pub trait QuerySetMember: Hash + Eq + Display + Sized + Clone + 'static {
    type Iter: ExactSizeIterator<Item = Self>;
    fn all() -> Self::Iter;
}

/// A `QuerySet` represents an allowed set of values for a field in a
/// result set. It can be turned into a URL query pair for Django
/// style queries, where depending on the number of values, we may
/// want to match the field value directly, or use a mangled field
/// name to indicate we want a set operation to be performed. Note
/// that unless `include()` or `exclude()` is called before `query()`,
/// no terms will be added to the filtering for the result set by this
/// set (i.e. the initial value indicates that all values are
/// acceptable).
pub struct QuerySet<Q: QuerySetMember> {
    /// `values` is initially unset, indicating no filtering on this
    /// field has been requested.
    values: Option<HashSet<Q>>,
    /// This is the remote name to query. It has to be stored here,
    /// because we'll need to mangle it in some cases.
    field_name: String,
}

impl<Q: QuerySetMember> QuerySet<Q> {
    /// `field_name` should be the base Django field name,
    /// e.g. "state"; any required variations like "state__in" will be
    /// created from this automatically when `query()` is called.
    pub fn new(field_name: String) -> Self {
        QuerySet {
            values: None,
            field_name,
        }
    }

    /// Request that a value be included in the result set. If this is
    /// the first call to `include()` or `exclude()` for this query
    /// set, the set of allowable values is narrowed to just
    /// `value`. On any call but the first, or if `exclude()` has been
    /// previous called, `include()` adds the value to the value set
    /// if it is not present, but does not remove any previously matched values.
    pub fn include(&mut self, value: Q) -> &mut Self {
        self.values.get_or_insert_with(HashSet::new).insert(value);
        self
    }

    /// Request that a value be excluded from the result set. This function
    /// can be called repeatedly, and can be freely mixed with `include()`. Note
    /// that due to the semantics of the first call to `include()`, the result
    /// set of
    /// `
    ///   qs.exclude(E).include(E);
    /// `
    /// is different from the result set of
    /// `
    ///   qs.include(E).exclude(E);
    /// `
    /// where the former includes all values, and the latter includes no values.
    pub fn exclude(&mut self, value: &Q) -> &mut Self {
        self.values
            .get_or_insert_with(|| Q::all().collect::<HashSet<_>>())
            .remove(value);
        self
    }

    /// Return a key-value pair suitable for inclusion in a URL query
    /// string, which will match the values requested so far. It
    /// returns `None` when there is no need to include anything in
    /// the URL for this query set. Otherwise it returns
    /// `Some((key,value))`. Note that `key` may not be equal to the
    /// field name provided at construction time, as Django maps
    /// operators other than equals to pseudo-fields with predictable
    /// names.
    pub fn query(&self) -> Option<(String, String)> {
        if let Some(values) = &self.values {
            match values.len() {
                0 => Some((format!("{}__in", self.field_name), String::new())),
                1 => Some((
                    self.field_name.clone(),
                    values.iter().next().unwrap().to_string(),
                )),
                _ if values.len() == Q::all().len() => None,
                _ => Some((
                    format!("{}__in", self.field_name),
                    values
                        .iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<String>>()
                        .join(","),
                )),
            }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::{Formatter, Result};
    use strum::{EnumIter, IntoEnumIterator};

    #[derive(Clone, Debug, Hash, PartialEq, Eq, EnumIter)]
    enum Test1 {
        State1,
        State2,
        State3,
    }

    impl Display for Test1 {
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            match self {
                Test1::State1 => write!(f, "State1"),
                Test1::State2 => write!(f, "State2"),
                Test1::State3 => write!(f, "State3"),
            }
        }
    }

    impl QuerySetMember for Test1 {
        type Iter = Test1Iter;
        fn all() -> Self::Iter {
            Self::iter()
        }
    }

    #[derive(Clone, Debug, Hash, PartialEq, Eq, EnumIter)]
    enum Test2 {
        State1,
        State2,
        State3,
        State4,
        State5,
    }

    impl Display for Test2 {
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            match self {
                Test2::State1 => write!(f, "State1"),
                Test2::State2 => write!(f, "State2"),
                Test2::State3 => write!(f, "State3"),
                Test2::State4 => write!(f, "State4"),
                Test2::State5 => write!(f, "State5"),
            }
        }
    }

    impl QuerySetMember for Test2 {
        type Iter = Test2Iter;
        fn all() -> Self::Iter {
            Self::iter()
        }
    }

    #[test]
    fn test_query_set() {
        // The default value yields no query
        let pair = QuerySet::<Test1>::new(String::from("test1")).query();
        assert!(pair.is_none());

        // An individual item gives a Django single value query
        let pair = QuerySet::new(String::from("test2"))
            .include(Test2::State4)
            .query();
        assert!(pair.is_some());
        let (field, value) = pair.unwrap();
        assert_eq!(field, "test2");
        assert_eq!(value, "State4");

        // A pair of items gives a set query
        let pair = QuerySet::new(String::from("test1"))
            .include(Test1::State1)
            .include(Test1::State2)
            .query();

        assert!(pair.is_some());
        let (field, value) = pair.unwrap();
        assert_eq!(field, "test1__in");
        assert!(value == "State1,State2" || value == "State2,State1");

        // Including all items explicitly takes us back to no query
        let pair = QuerySet::new(String::from("test1"))
            .include(Test1::State1)
            .include(Test1::State2)
            .include(Test1::State3)
            .query();
        assert!(pair.is_none());

        // Excluding one item gives us a set query
        let pair = QuerySet::new(String::from("test1"))
            .exclude(&Test1::State1)
            .query();

        assert!(pair.is_some());
        let (field, value) = pair.unwrap();
        assert_eq!(field, "test1__in");
        assert!(value == "State2,State3" || value == "State3,State2");

        // Excluding all but one item gives us a single value query
        let pair = QuerySet::new(String::from("test2"))
            .exclude(&Test2::State1)
            .exclude(&Test2::State2)
            .exclude(&Test2::State4)
            .exclude(&Test2::State5)
            .query();
        let (field, value) = pair.unwrap();
        assert_eq!(field, "test2");
        assert_eq!(value, "State3");

        // Excluding all items gives us an empty set query
        let pair = QuerySet::new(String::from("test1"))
            .exclude(&Test1::State1)
            .exclude(&Test1::State2)
            .exclude(&Test1::State3)
            .query();
        assert!(pair.is_some());
        let (field, value) = pair.unwrap();
        assert_eq!(field, "test1__in");
        assert_eq!(value, "");

        // Including and then excluding an item gives us the empty set
        let pair = QuerySet::new(String::from("test1"))
            .include(Test1::State1)
            .exclude(&Test1::State1)
            .query();
        assert!(pair.is_some());
        let (field, value) = pair.unwrap();
        assert_eq!(field, "test1__in");
        assert_eq!(value, "");

        // Excluding and then including an item gives us the complete set
        let pair = QuerySet::new(String::from("test2"))
            .exclude(&Test2::State5)
            .include(Test2::State5)
            .query();
        assert!(pair.is_none());
    }
}
