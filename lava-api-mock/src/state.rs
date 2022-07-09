use super::{
    Alias, Architecture, BitWidth, Core, Device, DeviceType, Group, Job, ProcessorFamily, Tag,
    TestCase, TestSet, TestSuite, User, Worker,
};

use boulder::{
    Buildable, Builder, GeneratableWithPersianRug, GeneratorWithPersianRug,
    GeneratorWithPersianRugIterator, GeneratorWithPersianRugMutIterator, RepeatFromPersianRug,
    SubsetsFromPersianRug, TryRepeatFromPersianRug,
};
use clone_replace::{CloneReplace, MutateGuard};
use django_query::mock::clone_replace::persian_rug::CloneReplacePersianRugTableSource;
use django_query::mock::{EndpointWithContext, NestedEndpointParams, NestedEndpointWithContext};
use persian_rug::{Context, Mutator, Proxy};
use std::sync::Arc;

/// The data backing a mock Lava instance
///
/// This is a [`persian_rug::Context`] containing all of the different
/// data types that make up the database of a Lava instance.
#[derive(Clone, Debug, Default)]
#[persian_rug::persian_rug]
pub struct State {
    #[table]
    aliases: Alias<State>,
    #[table]
    architectures: Architecture<State>,
    #[table]
    bit_widths: BitWidth<State>,
    #[table]
    cores: Core<State>,
    #[table]
    devices: Device<State>,
    #[table]
    device_types: DeviceType<State>,
    #[table]
    groups: Group<State>,
    #[table]
    jobs: Job<State>,
    #[table]
    processor_family: ProcessorFamily<State>,
    #[table]
    tags: Tag<State>,
    #[table]
    test_cases: TestCase<State>,
    #[table]
    test_sets: TestSet<State>,
    #[table]
    test_suites: TestSuite<State>,
    #[table]
    users: User<State>,
    #[table]
    workers: Worker<State>,
}

/// A thin wrapper around [`State`] for shared access.
///
/// Although a [`State`] can hold all the necessary data, it doesn't
/// define a strategy for sharing that data so it can be
/// updated. Owing to limitations in the underlying crates this crate
/// is based on, there's only really one sensible way to do this at
/// present, and that's to use a [`CloneReplace`] to hold the data.
///
/// This is just a lightweight wrapper with some convenient methods to
/// allow you to create [`wiremock`] endpoints.  Those are in turn
/// based on [`EndpointWithContext`] from [`django_query`]
/// (specifically this is the `WithContext` variant, because the
/// connections between the different data types are handled using
/// [`persian-rug`](persian_rug), and in fact a [`State`] is just a
/// [`persian_rug::Context`].
pub struct SharedState(CloneReplace<State>);

impl SharedState {
    /// Create and wrap a new empty [`State`].
    ///
    /// Example:
    /// ```rust
    /// use lava_api_mock::SharedState;
    ///
    /// let p = SharedState::new();
    /// ```
    pub fn new() -> Self {
        Self(CloneReplace::new(State::new()))
    }

    /// Create, populate and wrap a [`State`].
    ///
    /// `pop` is a [`PopulationParams`] instance giving a count for
    /// each type of object.
    ///
    /// Example:
    /// ```rust
    /// use lava_api_mock::SharedState;
    ///
    /// let p = SharedState::new_populated(Default::default());
    /// ```
    pub fn new_populated(pop: PopulationParams) -> Self {
        Self(CloneReplace::new(State::new_populated(pop)))
    }

    /// Create a new [`EndpointWithContext`] for type `T` within the
    /// enclosed [`State`].
    ///
    /// The return value is an implementor of [`wiremock::Respond`] and can
    /// be mounted directly onto a wiremock server instance.
    ///
    /// Example:
    /// ```rust
    /// use lava_api_mock::{Job, State, SharedState};
    ///
    /// # tokio_test::block_on( async {
    /// let p = SharedState::new();
    ///
    /// let server = wiremock::MockServer::start().await;
    ///
    /// wiremock::Mock::given(wiremock::matchers::method("GET"))
    ///     .and(wiremock::matchers::path("/api/v0.2/jobs/"))
    ///     .respond_with(p.endpoint::<Job<State>>(Some(&server.uri()), None))
    ///     .mount(&server)
    ///     .await;
    /// # });
    /// ```
    pub fn endpoint<T>(
        &self,
        uri: Option<&str>,
        default_limit: Option<usize>,
    ) -> EndpointWithContext<
        CloneReplacePersianRugTableSource<
            impl Fn(&Arc<State>) -> persian_rug::TableIterator<'_, T> + Clone,
            State,
        >,
    >
    where
        T: persian_rug::Contextual<Context = State> + 'static,
        State: persian_rug::Owner<T>,
    {
        let mut ep = EndpointWithContext::new(
            CloneReplacePersianRugTableSource::new(
                self.0.clone(),
                |s: &Arc<State>| -> persian_rug::TableIterator<'_, T> { s.get_iter() },
            ),
            uri,
        );
        if let Some(default_limit) = default_limit {
            ep.default_limit(default_limit);
        }
        ep
    }

    /// Create a new [`NestedEndpointWithContext`] for type `T` within the
    /// enclosed [`State`].
    ///
    /// Nested endpoints objects data that can only be queried by
    /// providing some related object, like finding [`TestCase`]
    /// instances that match a given [`Job`] for example: here `tests`
    /// is nested under `jobs`. See the documentation for
    /// [`NestedEndpointWithContext`] for more details.
    ///
    /// The return value is an implementor of [`wiremock::Respond`] and can
    /// be mounted directly onto a wiremock server instance.
    ///
    /// Example:
    /// ```rust
    /// use django_query::mock::{nested_endpoint_matches, NestedEndpointParams};
    /// use lava_api_mock::{Job, State, SharedState, TestCase};
    ///
    /// let p = SharedState::new();
    ///
    /// # tokio_test::block_on( async {
    /// let server = wiremock::MockServer::start().await;
    ///
    /// wiremock::Mock::given(wiremock::matchers::method("GET"))
    ///     .and(nested_endpoint_matches("/api/v0.2", "jobs", "tests"))
    ///     .respond_with(p.nested_endpoint::<TestCase<State>>(
    ///         NestedEndpointParams {
    ///             root: "/api/v0.2",
    ///             parent: "jobs",
    ///             child: "tests",
    ///             parent_query: "suite__job__id",
    ///             base_uri: Some(&server.uri()),
    ///         },
    ///         Some(10),
    ///     ))
    ///     .mount(&server)
    ///     .await;
    /// # });
    /// ```
    pub fn nested_endpoint<T>(
        &self,
        params: NestedEndpointParams<'_>,
        default_limit: Option<usize>,
    ) -> NestedEndpointWithContext<
        CloneReplacePersianRugTableSource<
            impl Fn(&Arc<State>) -> persian_rug::TableIterator<'_, T> + Clone,
            State,
        >,
    >
    where
        T: persian_rug::Contextual<Context = State> + 'static,
        State: persian_rug::Owner<T>,
    {
        let mut ep = NestedEndpointWithContext::new(
            CloneReplacePersianRugTableSource::new(
                self.0.clone(),
                |s: &Arc<State>| -> persian_rug::TableIterator<'_, T> { s.get_iter() },
            ),
            params,
        );
        if let Some(default_limit) = default_limit {
            ep.default_limit(default_limit);
        }
        ep
    }

    /// Obtain a [`persian_rug::Accessor`] for the enclosed [`State`]
    ///
    /// This permits reading the data contained in the [`State`].
    ///
    /// Example:
    /// ```rust
    /// use lava_api_mock::{Job, SharedState};
    /// use persian_rug::Accessor;
    ///
    /// let p = SharedState::new_populated(Default::default());
    ///
    /// for job in p.access().get_proxy_iter::<Job<_>>() {
    ///     println!("Got job {:?}", p.access().get(&job));
    /// }
    /// ```
    pub fn access(&self) -> Arc<State> {
        self.0.access()
    }

    /// Obtain a [`persian_rug::Mutator`] for the enclosed [`State`]
    ///
    /// This permits modifying the data contained in the [`State`].
    ///
    /// Example:
    /// ```rust
    /// use boulder::{BuildableWithPersianRug, BuilderWithPersianRug};
    /// use lava_api_mock::{Job, SharedState, State};
    /// use persian_rug::Proxy;
    ///
    /// let mut p = SharedState::new_populated(Default::default());
    ///
    /// let _ = Proxy::<Job<State>>::builder().build(p.mutate());
    /// ```
    pub fn mutate(&mut self) -> MutateGuard<State> {
        self.0.mutate()
    }
}

impl Clone for SharedState {
    fn clone(&self) -> Self {
        SharedState(self.0.clone())
    }
}

impl Default for SharedState {
    fn default() -> Self {
        Self::new()
    }
}

/// Initial population sizes for the data in a [`State`]
///
/// This specifies the number of objects of each type to
/// generate when initializing a [`State`] instance using
/// [`new_populated`](State::new_populated). It is
/// [`Buildable`] so you can customise just some fields
/// from default if you.
///
/// The default values are:
/// - 10 [`Alias`] instances
/// - 5 [`Architecture`] instances
/// - 2 [`BitWidth`] instances
/// - 3 [`Core`] instances
/// - 50 [`Device`] instances
/// - 10 [`DeviceType`] instances
/// - 3 [`Group`] instances
/// - 200 [`Job`] instances
/// - 3 [`ProcessorFamily`] instances
/// - 5 [`Tag`] instances
/// - 5 [`User`] instances
/// - 10 [`Worker`] instances
///
/// It also asks for:
/// - 5 [`TestCase`] instances
/// - 2 [`TestSet`] instances
/// - 3 [`TestSuite`] instances
/// to be created for each job that is created.
#[derive(Buildable, Clone, Debug, Eq, PartialEq)]
pub struct PopulationParams {
    #[boulder(default = 10usize)]
    pub aliases: usize,
    #[boulder(default = 5usize)]
    pub architectures: usize,
    #[boulder(default = 2usize)]
    pub bit_widths: usize,
    #[boulder(default = 3usize)]
    pub cores: usize,
    #[boulder(default = 50usize)]
    pub devices: usize,
    #[boulder(default = 10usize)]
    pub device_types: usize,
    #[boulder(default = 3usize)]
    pub groups: usize,
    #[boulder(default = 200usize)]
    pub jobs: usize,
    #[boulder(default = 3usize)]
    pub processor_families: usize,
    #[boulder(default = 5usize)]
    pub tags: usize,
    #[boulder(default = 5usize)]
    pub test_cases: usize,
    #[boulder(default = 2usize)]
    pub test_sets: usize,
    #[boulder(default = 3usize)]
    pub test_suites: usize,
    #[boulder(default = 5usize)]
    pub users: usize,
    #[boulder(default = 10usize)]
    pub workers: usize,
}

impl PopulationParams {
    /// Create a new default [`PopulationParams`]
    ///
    /// This is equivalent to using the [`Builder`] without
    /// customising it.
    ///
    /// ```rust
    /// use boulder::{Buildable, Builder};
    /// use lava_api_mock::PopulationParams;
    ///
    /// assert_eq!(PopulationParams::new(), PopulationParams::builder().build());
    /// ```
    pub fn new() -> Self {
        Default::default()
    }
}

impl Default for PopulationParams {
    fn default() -> Self {
        Self::builder().build()
    }
}

struct JobGenerator {
    job: Option<Proxy<Job<State>>>,
}

impl JobGenerator {
    pub fn new(job: Option<Proxy<Job<State>>>) -> Self {
        Self { job }
    }
}

impl GeneratorWithPersianRug<State> for JobGenerator {
    type Output = Proxy<Job<State>>;

    fn generate<'b, B>(&mut self, context: B) -> (Self::Output, B)
    where
        B: 'b + Mutator<Context = State>,
    {
        (self.job.unwrap(), context)
    }
}

struct SuiteGenerator {
    suite: usize,
    suites: Vec<Proxy<TestSuite<State>>>,
}

impl SuiteGenerator {
    pub fn new(suites: Vec<Proxy<TestSuite<State>>>) -> Self {
        SuiteGenerator { suite: 0, suites }
    }
}

impl GeneratorWithPersianRug<State> for SuiteGenerator {
    type Output = Proxy<TestSuite<State>>;

    fn generate<'b, B>(&mut self, context: B) -> (Self::Output, B)
    where
        B: 'b + Mutator<Context = State>,
    {
        let suite = self.suites[self.suite];
        self.suite = (self.suite + 1) % self.suites.len();

        (suite, context)
    }
}

struct SetGenerator {
    suite: usize,
    set: usize,
    suites: Vec<Proxy<TestSuite<State>>>,
    sets: Vec<Proxy<TestSet<State>>>,
}

impl SetGenerator {
    fn new(suites: Vec<Proxy<TestSuite<State>>>, sets: Vec<Proxy<TestSet<State>>>) -> Self {
        SetGenerator {
            suite: 0,
            set: 0,
            suites,
            sets,
        }
    }
}

impl GeneratorWithPersianRug<State> for SetGenerator {
    type Output = Option<Proxy<TestSet<State>>>;

    fn generate<'b, B>(&mut self, context: B) -> (Self::Output, B)
    where
        B: 'b + Mutator<Context = State>,
    {
        if self.suites.is_empty() || self.sets.is_empty() {
            return (None, context);
        }

        let suite = self.suites[self.suite];
        self.suite = (self.suite + 1) % self.suites.len();

        let mut attempts = 0;
        let set = loop {
            let set = self.sets[self.set];
            self.set = (self.set + 1) % self.sets.len();
            attempts += 1;
            if context.get(&set).suite == suite {
                break Some(set);
            }
            if attempts == self.sets.len() {
                break None;
            }
        };

        (set, context)
    }
}

impl State {
    /// Create a new empty [`State`]
    pub fn new() -> Self {
        Default::default()
    }

    /// A [`DeviceType`] [`GeneratorWithPersianRug`] that uses
    /// dependencies already in the [`State`].
    ///
    /// This generator is equivalent to the default, except that it
    /// draws [`Alias`], [`Architecture`], [`BitWidth`], [`Core`] and
    /// [`ProcessorFamily`] instances from those already in the
    /// containing [`State`] at the point of generation.
    pub fn make_device_type_generator(
    ) -> impl GeneratorWithPersianRug<State, Output = Proxy<DeviceType<State>>> {
        Proxy::<DeviceType<State>>::generator()
            .aliases(SubsetsFromPersianRug::new())
            .architecture(TryRepeatFromPersianRug::new())
            .bits(TryRepeatFromPersianRug::new())
            .cores(SubsetsFromPersianRug::new())
            .processor(TryRepeatFromPersianRug::new())
    }

    /// A [`User`] [`GeneratorWithPersianRug`] that uses
    /// dependencies already in the [`State`].
    ///
    /// This generator is equivalent to the default, except that it
    /// draws [`Group`] instances from those already in the containing
    /// [`State`] at the point of generation.
    pub fn make_user_generator() -> impl GeneratorWithPersianRug<State, Output = Proxy<User<State>>>
    {
        Proxy::<User<State>>::generator().group(TryRepeatFromPersianRug::new())
    }

    /// A [`Device`] [`GeneratorWithPersianRug`] that uses
    /// dependencies already in the [`State`].
    ///
    /// This generator is equivalent to the default, except that it
    /// draws [`DeviceType`], [`User`], [`Group`],
    /// [`Tag`] and [`Worker`] instances from those already in
    /// the containing [`State`] at the point of generation.
    pub fn make_device_generator(
    ) -> impl GeneratorWithPersianRug<State, Output = Proxy<Device<State>>> {
        Proxy::<Device<State>>::generator()
            .device_type(RepeatFromPersianRug::new())
            .physical_owner(TryRepeatFromPersianRug::new())
            .physical_group(TryRepeatFromPersianRug::new())
            .tags(SubsetsFromPersianRug::new())
            .worker_host(RepeatFromPersianRug::new())
    }

    /// A [`Job`] [`GeneratorWithPersianRug`] that uses
    /// dependencies already in the [`State`].
    ///
    /// This generator is equivalent to the default, except that it
    /// draws [`User`], [`Group`], [`DeviceType`], [`Tag`] and
    /// [`Device`] instances from those already in the
    /// containing [`State`] at the point of generation.
    pub fn make_job_generator() -> impl GeneratorWithPersianRug<State, Output = Proxy<Job<State>>> {
        Proxy::<Job<State>>::generator()
            .submitter(RepeatFromPersianRug::new())
            .viewing_groups(SubsetsFromPersianRug::new())
            .requested_device_type(TryRepeatFromPersianRug::new())
            .tags(SubsetsFromPersianRug::new())
            .actual_device(TryRepeatFromPersianRug::new())
    }

    /// Create a new [`State`] with some initial data.
    ///
    /// Here, `pop` is a [`PopulationParams`] which gives the initial
    /// number of each type of object. The object generators are
    /// customised to draw their references from the other objects in
    /// the state.
    ///
    /// You can obtain new instances of themodified generators from
    /// [`make_device_generator`](State::make_device_generator),
    /// [`make_device_type_generator`](State::make_device_type_generator),
    /// [`make_job_generator`](State::make_job_generator) and
    /// [`make_user_generator`](State::make_user_generator) if you
    /// need to create more objects in a similar fashion.
    ///
    /// Note that because tests are per-job objects, the counts in
    /// [`PopulationParams`] for [`TestCase`], [`TestSet`] and
    /// [`TestSuite`] are used to make custom objects for each job.
    /// The tests are not provided automatically when jobs are generated
    /// by the underlying [`GeneratorWithPersianRug`] provided by
    /// [`make_job_generator`](State::make_job_generator).
    pub fn new_populated(pop: PopulationParams) -> Self {
        let mut s: State = Default::default();

        let aliases = Proxy::<Alias<State>>::generator();
        let _ = GeneratorWithPersianRugIterator::new(aliases, &mut s)
            .take(pop.aliases)
            .collect::<Vec<_>>();

        let architectures = Proxy::<Architecture<State>>::generator();
        let _ = GeneratorWithPersianRugIterator::new(architectures, &mut s)
            .take(pop.architectures)
            .collect::<Vec<_>>();

        let bit_widths = Proxy::<BitWidth<State>>::generator();
        let _ = GeneratorWithPersianRugIterator::new(bit_widths, &mut s)
            .take(pop.bit_widths)
            .collect::<Vec<_>>();

        let cores = Proxy::<Core<State>>::generator();
        let _ = GeneratorWithPersianRugIterator::new(cores, &mut s)
            .take(pop.cores)
            .collect::<Vec<_>>();

        let processor_families = Proxy::<ProcessorFamily<State>>::generator();
        let _ = GeneratorWithPersianRugIterator::new(processor_families, &mut s)
            .take(pop.processor_families)
            .collect::<Vec<_>>();

        let device_types = Self::make_device_type_generator();
        let _ = GeneratorWithPersianRugIterator::new(device_types, &mut s)
            .take(pop.device_types)
            .collect::<Vec<_>>();

        let groups = Proxy::<Group<State>>::generator();
        let _ = GeneratorWithPersianRugIterator::new(groups, &mut s)
            .take(pop.groups)
            .collect::<Vec<_>>();

        let users = Self::make_user_generator();
        let _ = GeneratorWithPersianRugIterator::new(users, &mut s)
            .take(pop.users)
            .collect::<Vec<_>>();

        let workers = Proxy::<Worker<State>>::generator();
        let _ = GeneratorWithPersianRugIterator::new(workers, &mut s)
            .take(pop.workers)
            .collect::<Vec<_>>();

        let tags = Proxy::<Tag<State>>::generator();
        let _ = GeneratorWithPersianRugIterator::new(tags, &mut s)
            .take(pop.tags)
            .collect::<Vec<_>>();

        let devices = Self::make_device_generator();
        let _ = GeneratorWithPersianRugIterator::new(devices, &mut s)
            .take(pop.devices)
            .collect::<Vec<_>>();

        let jobs = Self::make_job_generator();
        let jobs = GeneratorWithPersianRugIterator::new(jobs, &mut s)
            .take(pop.jobs)
            .collect::<Vec<_>>();

        let mut suites = Proxy::<TestSuite<State>>::generator().job(JobGenerator::new(None));
        let mut sets = Proxy::<TestSet<State>>::generator().suite(SuiteGenerator::new(Vec::new()));
        let mut cases = Proxy::<TestCase<State>>::generator()
            .suite(SuiteGenerator::new(Vec::new()))
            .test_set(SetGenerator::new(Vec::new(), Vec::new()));

        for job in jobs {
            suites = suites.job(JobGenerator::new(Some(job)));
            let suites = GeneratorWithPersianRugMutIterator::new(&mut suites, &mut s)
                .take(pop.test_suites)
                .collect::<Vec<_>>();

            sets = sets.suite(SuiteGenerator::new(suites.clone()));
            let sets = GeneratorWithPersianRugMutIterator::new(&mut sets, &mut s)
                .take(pop.test_sets)
                .collect::<Vec<_>>();

            cases = cases
                .suite(SuiteGenerator::new(suites.clone()))
                .test_set(SetGenerator::new(suites.clone(), sets.clone()));
            let _ = GeneratorWithPersianRugMutIterator::new(&mut cases, &mut s)
                .take(pop.test_cases)
                .collect::<Vec<_>>();
        }

        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{JobState, SharedState};

    use anyhow::Result;
    use boulder::{BuildableWithPersianRug, BuilderWithPersianRug};
    use persian_rug::Proxy;
    use serde_json::{json, Value};

    async fn make_request<T, U>(server_uri: T, endpoint: U) -> Result<Value>
    where
        T: AsRef<str>,
        U: AsRef<str>,
    {
        let url = format!("{}/api/v0.2/{}", server_uri.as_ref(), endpoint.as_ref());
        Ok(reqwest::get(&url).await?.json().await?)
    }

    #[tokio::test]
    async fn test_state() {
        let mut p = SharedState::new();
        {
            let m = p.mutate();
            let (_, m) = Proxy::<Job<State>>::builder().id(100).build(m);
            let (_, _m) = Proxy::<Job<State>>::builder().id(101).build(m);
        }

        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/v0.2/jobs/"))
            .respond_with(p.endpoint::<Job<State>>(Some(&server.uri()), None))
            .mount(&server)
            .await;

        let jobs = make_request(server.uri(), "jobs/")
            .await
            .expect("failed to query jobs");

        assert_eq!(jobs["results"][0]["id"], json!(100));
        assert_eq!(jobs["results"][1]["id"], json!(101));
        assert_eq!(jobs["results"].as_array().unwrap().len(), 2);

        {
            let m = p.mutate();
            let (_, _m) = Proxy::<Job<State>>::builder()
                .id(102)
                .state(JobState::Submitted)
                .build(m);
        }

        let jobs = make_request(server.uri(), "jobs/")
            .await
            .expect("failed to query jobs");

        assert_eq!(jobs["results"][0]["id"], json!(100));
        assert_eq!(jobs["results"][1]["id"], json!(101));
        assert_eq!(jobs["results"][2]["id"], json!(102));
        assert_eq!(jobs["results"].as_array().unwrap().len(), 3);

        {
            let mut m = p.mutate();
            for j in m.get_iter_mut::<Job<State>>() {
                if j.id == 102 {
                    j.state = JobState::Finished
                }
            }
        }

        let jobs = make_request(server.uri(), "jobs/")
            .await
            .expect("failed to query jobs");

        assert_eq!(jobs["results"][0]["id"], json!(100));
        assert_eq!(jobs["results"][1]["id"], json!(101));
        assert_eq!(jobs["results"][2]["id"], json!(102));
        assert_eq!(jobs["results"][2]["state"], json!("Finished"));
        assert_eq!(jobs["results"].as_array().unwrap().len(), 3);
    }
}
