use crate::{JobHealth, JobState, Server, SharedState, State};

use boulder::{
    GeneratableWithPersianRug, GeneratorWithPersianRug, RepeatFromPersianRug,
    SubsetsFromPersianRug, TryRepeatFromPersianRug,
};
use chrono::{DateTime, Utc};
use clocks::Clock;
use clone_replace::MutateGuard;
use num::NumCast;
use persian_rug::{Accessor, Mutator, Proxy};
use std::collections::BTreeMap;

type Device = crate::Device<State>;
type DeviceType = crate::DeviceType<State>;
type Job = crate::Job<State>;
type Tag = crate::Tag<State>;
type Worker = crate::Worker<State>;

pub trait Generator {
    type Output;
    fn generate(&mut self, context: MutateGuard<State>) -> (Self::Output, MutateGuard<State>);
}

impl<T> Generator for T
where
    T: boulder::GeneratorWithPersianRug<State>,
{
    type Output = <T as boulder::GeneratorWithPersianRug<State>>::Output;

    fn generate(&mut self, context: MutateGuard<State>) -> (Self::Output, MutateGuard<State>) {
        <T as boulder::GeneratorWithPersianRug<State>>::generate(self, context)
    }
}

impl<T> Generator for Box<dyn Generator<Output = T> + 'static> {
    type Output = T;
    fn generate(&mut self, context: MutateGuard<State>) -> (Self::Output, MutateGuard<State>) {
        self.as_mut().generate(context)
    }
}

pub trait GeneratorExt: Generator {
    fn take_n(&mut self, context: MutateGuard<State>, count: usize) -> TakeNIterator<Self> {
        TakeNIterator::new(self, context, count)
    }
}

impl<T> GeneratorExt for T where T: Generator {}

struct IdGenerator<T, U: NumCast> {
    _type_marker: core::marker::PhantomData<T>,
    _out_marker: core::marker::PhantomData<U>,
}

impl<T, U: NumCast> IdGenerator<T, U> {
    pub fn new() -> Self {
        Self {
            _type_marker: Default::default(),
            _out_marker: Default::default(),
        }
    }
}

#[persian_rug::constraints(context=C, access(T))]
impl<C, T, U> GeneratorWithPersianRug<C> for IdGenerator<T, U>
where
    U: NumCast,
{
    type Output = U;

    fn generate<'b, B>(&mut self, context: B) -> (U, B)
    where
        B: 'b + Mutator<Context = C>,
    {
        (num::cast(context.get_iter::<T>().count()).unwrap(), context)
    }
}

pub async fn create_mock(now: DateTime<Utc>) -> (Mock, Clock<Utc>) {
    let clock = Clock::new_fake(now);
    (Mock::new_with_clock(clock.clone()).await, clock)
}

pub struct TakeNIterator<'a, G>
where
    G: Generator + ?Sized,
{
    count: usize,
    context: Option<MutateGuard<State>>,
    generator: &'a mut G,
}

impl<'a, G> TakeNIterator<'a, G>
where
    G: Generator + ?Sized,
{
    pub fn new(generator: &'a mut G, context: MutateGuard<State>, count: usize) -> Self {
        Self {
            count,
            context: Some(context),
            generator,
        }
    }

    #[allow(dead_code)]
    pub fn into_inner(self) -> MutateGuard<State> {
        self.context.unwrap()
    }
}

impl<'a, G> Iterator for TakeNIterator<'a, G>
where
    G: Generator + ?Sized,
{
    type Item = <G as Generator>::Output;

    fn next(&mut self) -> Option<Self::Item> {
        if self.count == 0usize {
            None
        } else {
            let (result, context) = self.generator.generate(self.context.take().unwrap());
            self.context = Some(context);
            self.count -= 1;
            Some(result)
        }
    }
}

/// A simplified API for using the mock crate
///
/// This integrates four major pieces that you can construct
/// separately (and much more flexibly) to make many test cases easier
/// to write.
///
/// - A [SharedState] that holds the actual data.
/// - A [Server] that exposes a REST API on some port.
/// - A [Clock] that you can synchronise to in test cases to
///   reproduce timing-critical issues.
/// - A set of [Generator] instances for producing new
///   jobs, tags, devices, device types and workers.
pub struct Mock {
    state: SharedState,
    server: Server,
    clock: Clock<Utc>,

    devices_lut: BTreeMap<String, Proxy<Device>>,
    device_types_lut: BTreeMap<String, Proxy<DeviceType>>,
    jobs_lut: BTreeMap<i64, Proxy<Job>>,
    tags_lut: BTreeMap<String, Proxy<Tag>>,
    workers_lut: BTreeMap<String, Proxy<Worker>>,

    devices: Box<dyn Generator<Output = Proxy<Device>>>,
    device_types: Box<dyn Generator<Output = Proxy<DeviceType>>>,
    jobs: Box<dyn Generator<Output = Proxy<Job>>>,
    tags: Box<dyn Generator<Output = Proxy<Tag>>>,
    workers: Box<dyn Generator<Output = Proxy<Worker>>>,

    bulk_devices: Box<dyn Generator<Output = Proxy<Device>>>,
    bulk_device_types: Box<dyn Generator<Output = Proxy<DeviceType>>>,
    bulk_jobs: Box<dyn Generator<Output = Proxy<Job>>>,
    bulk_tags: Box<dyn Generator<Output = Proxy<Tag>>>,
    bulk_workers: Box<dyn Generator<Output = Proxy<Worker>>>,
}

impl Mock {
    /// Create a new mock
    ///
    /// The mock's clock will be a wall clock.
    pub async fn new() -> Self {
        Self::new_with_clock(Default::default()).await
    }

    /// Create a new mock with the given clock
    ///
    /// The mock will use the clock given.
    pub async fn new_with_clock(clock: Clock<Utc>) -> Self {
        let mut s = SharedState::new();
        let c = clock.clone();
        let c2 = clock.clone();

        let mut g = Proxy::<crate::User<State>>::generator();
        for _ in g.take_n(s.mutate(), 10) {}

        Self {
            state: s.clone(),
            server: Server::new(s, Default::default()).await,

            clock,

            devices_lut: BTreeMap::new(),
            device_types_lut: BTreeMap::new(),
            jobs_lut: BTreeMap::new(),
            tags_lut: BTreeMap::new(),
            workers_lut: BTreeMap::new(),

            devices: Box::new(
                Proxy::<Device>::generator()
                    .device_type(RepeatFromPersianRug::new())
                    .physical_owner(TryRepeatFromPersianRug::new())
                    .physical_group(TryRepeatFromPersianRug::new())
                    .tags(SubsetsFromPersianRug::new())
                    .health(|| crate::DeviceHealth::Good)
                    .state(|| crate::DeviceState::Idle)
                    .worker_host(RepeatFromPersianRug::new()),
            ),
            device_types: Box::new(Proxy::<DeviceType>::generator()),
            jobs: Box::new(
                Proxy::<Job>::generator()
                    .id(IdGenerator::<Job, _>::new())
                    .submitter(RepeatFromPersianRug::new())
                    .viewing_groups(SubsetsFromPersianRug::new())
                    .requested_device_type(TryRepeatFromPersianRug::new())
                    .tags(SubsetsFromPersianRug::new())
                    .submit_time(move || Some(c.now()))
                    .start_time(|| None)
                    .end_time(|| None)
                    .state(|| JobState::Submitted)
                    .health(|| JobHealth::Unknown)
                    .actual_device(|| None),
            ),
            tags: Box::new(Proxy::<Tag>::generator().id(IdGenerator::<Tag, _>::new())),
            workers: Box::new(Proxy::<Worker>::generator()),

            bulk_devices: Box::new(Proxy::<Device>::generator()),
            bulk_device_types: Box::new(Proxy::<DeviceType>::generator()),
            bulk_jobs: Box::new(
                Proxy::<Job>::generator()
                    .id(IdGenerator::<Job, _>::new())
                    .submitter(RepeatFromPersianRug::new())
                    .viewing_groups(SubsetsFromPersianRug::new())
                    .requested_device_type(TryRepeatFromPersianRug::new())
                    .tags(SubsetsFromPersianRug::new())
                    .submit_time(move || Some(c2.now()))
                    .start_time(|| None)
                    .end_time(|| None)
                    .state(|| JobState::Submitted)
                    .health(|| JobHealth::Unknown)
                    .actual_device(|| None),
            ),
            bulk_tags: Box::new(Proxy::<Tag>::generator().id(IdGenerator::<Tag, _>::new())),
            bulk_workers: Box::new(Proxy::<Worker>::generator()),
        }
    }

    /// Get the URI for the mock server
    pub fn uri(&self) -> String {
        self.server.uri()
    }

    /// Execute a function on the data pointed to by the given proxy
    #[persian_rug::constraints(context = State, access(T))]
    pub fn with_proxy<T, F, R>(&self, proxy: &Proxy<T>, f: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        f(self.state.access().get(proxy))
    }

    /// Execute a function on the data pointed to by the given proxy
    #[persian_rug::constraints(context = State, access(T))]
    pub fn with_option_proxy<T, F, R>(&self, proxy: &Option<Proxy<T>>, f: F) -> R
    where
        F: FnOnce(Option<&T>) -> R,
    {
        let a = self.state.access();
        f(proxy.as_ref().map(|p| a.get(p)))
    }

    /// Execute a function on the mutable data pointed to by the given
    /// proxy
    #[persian_rug::constraints(context = State, access(T))]
    pub fn with_proxy_mut<T, F, R>(&mut self, proxy: &Proxy<T>, f: F) -> R
    where
        F: FnOnce(&mut T) -> R,
    {
        f(self.state.mutate().get_mut(proxy))
    }

    /// Execute a function on the mutable data pointed to by the given
    /// proxy
    #[persian_rug::constraints(context = State, access(T))]
    pub fn with_option_proxy_mut<T, F, R>(&mut self, proxy: &Option<Proxy<T>>, f: F) -> R
    where
        F: FnOnce(Option<&mut T>) -> R,
    {
        let mut m = self.state.mutate();
        f(proxy.as_ref().map(|p| m.get_mut(p)))
    }

    /// Get an [Accessor] for the [State] the mock holds
    ///
    /// This permits you to access an unchanging, read-only view of
    /// the data which the mock is currently serving.
    pub fn accessor(&self) -> impl Accessor<Context = State> {
        self.state.access()
    }

    /// Get an [Mutator] for the [State] the mock holds
    ///
    /// This permits you to mutate a writable copy of the data the
    /// mock holds. Note that modifications will only become visible
    /// when the mutator is dropped.
    pub fn mutator(&mut self) -> impl Mutator<Context = State> {
        self.state.mutate()
    }

    /// Execute a function on the device with the given hostname
    pub fn with_device<H, F, T>(&self, hostname: H, f: F) -> Option<T>
    where
        H: AsRef<str>,
        F: FnOnce(&Device) -> T,
    {
        self.devices_lut
            .get(hostname.as_ref())
            .map(|d| f(self.state.access().get(d)))
    }

    /// Execute a function on the mutable device with the given hostname
    pub fn with_device_mut<H, F, T>(&mut self, hostname: H, f: F) -> Option<T>
    where
        H: AsRef<str>,
        F: FnOnce(&mut Device) -> T,
    {
        self.devices_lut
            .get(hostname.as_ref())
            .map(|d| f(self.state.mutate().get_mut(d)))
    }

    /// Get the [Proxy] for the device with the given hostname
    pub fn get_device_proxy<H>(&self, hostname: H) -> Option<Proxy<Device>>
    where
        H: AsRef<str>,
    {
        self.devices_lut.get(hostname.as_ref()).copied()
    }

    /// Execute a function on each device
    pub fn with_devices<F>(&self, mut f: F)
    where
        F: FnMut(&Device),
    {
        for d in self.state.access().get_iter() {
            f(d)
        }
    }

    /// Execute a function on each mutable device
    pub fn with_devices_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut Device),
    {
        for d in self.state.mutate().get_iter_mut() {
            f(d)
        }
    }

    /// Execute a function on the device type with the given name
    pub fn with_device_type<N, F, T>(&self, name: N, f: F) -> Option<T>
    where
        N: AsRef<str>,
        F: FnOnce(&DeviceType) -> T,
    {
        self.device_types_lut
            .get(name.as_ref())
            .map(|dt| f(self.state.access().get(dt)))
    }

    /// Execute a function on the mutable device type with the given name
    pub fn with_device_type_mut<N, F, T>(&mut self, name: N, f: F) -> Option<T>
    where
        N: AsRef<str>,
        F: FnOnce(&mut DeviceType) -> T,
    {
        self.device_types_lut
            .get(name.as_ref())
            .map(|dt| f(self.state.mutate().get_mut(dt)))
    }

    /// Get the [Proxy] for the device type with the given name
    pub fn get_device_type_proxy<N>(&self, name: N) -> Option<Proxy<DeviceType>>
    where
        N: AsRef<str>,
    {
        self.device_types_lut.get(name.as_ref()).copied()
    }

    /// Execute a function on each device
    pub fn with_device_types<F>(&self, mut f: F)
    where
        F: FnMut(&DeviceType),
    {
        for d in self.state.access().get_iter() {
            f(d)
        }
    }

    /// Execute a function on each mutable device
    pub fn with_device_types_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut DeviceType),
    {
        for d in self.state.mutate().get_iter_mut() {
            f(d)
        }
    }

    /// Execute a function on the job with the given id
    pub fn with_job<F, T>(&self, id: i64, f: F) -> Option<T>
    where
        F: FnOnce(&Job) -> T,
    {
        self.jobs_lut
            .get(&id)
            .map(|j| f(self.state.access().get(j)))
    }

    /// Execute a function on the mutable job with the given id
    pub fn with_job_mut<F, T>(&mut self, id: i64, f: F) -> Option<T>
    where
        F: FnOnce(&mut Job) -> T,
    {
        self.jobs_lut
            .get(&id)
            .map(|j| f(self.state.mutate().get_mut(j)))
    }

    /// Get the [Proxy] for the job with the given id
    pub fn get_job_proxy(&self, job: i64) -> Option<Proxy<Job>> {
        self.jobs_lut.get(&job).copied()
    }

    /// Execute a function on each job
    pub fn with_jobs<F>(&self, mut f: F)
    where
        F: FnMut(&Job),
    {
        for d in self.state.access().get_iter() {
            f(d)
        }
    }

    /// Execute a function on each mutable job
    pub fn with_jobs_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut Job),
    {
        for d in self.state.mutate().get_iter_mut() {
            f(d)
        }
    }

    /// Execute a function on the tag with the given name
    pub fn with_tag<F, T>(&self, tag: &str, f: F) -> Option<T>
    where
        F: FnOnce(&Tag) -> T,
    {
        self.tags_lut
            .get(tag)
            .map(|t| f(self.state.access().get(t)))
    }

    /// Execute a function on the mutable tag with the given name
    pub fn with_tag_mut<F, T>(&mut self, tag: &str, f: F) -> Option<T>
    where
        F: FnOnce(&mut Tag) -> T,
    {
        self.tags_lut
            .get(tag)
            .map(|t| f(self.state.mutate().get_mut(t)))
    }

    /// Get the [Proxy] for the tag with the given id
    pub fn get_tag_proxy(&self, tag: &str) -> Option<Proxy<Tag>> {
        self.tags_lut.get(tag).copied()
    }

    /// Execute a function on every tag
    pub fn with_tags<F>(&self, mut f: F)
    where
        F: FnMut(&Tag),
    {
        for d in self.state.access().get_iter() {
            f(d)
        }
    }

    /// Execute a function on every mutable tag
    pub fn with_tags_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut Tag),
    {
        for d in self.state.mutate().get_iter_mut() {
            f(d)
        }
    }

    /// Execute a function on the worker with the given name
    pub fn with_worker<F, T>(&self, worker: &str, f: F) -> Option<T>
    where
        F: FnOnce(&Worker) -> T,
    {
        self.workers_lut
            .get(worker)
            .map(|t| f(self.state.access().get(t)))
    }

    /// Execute a function on the mutable worker with the given name
    pub fn with_worker_mut<F, T>(&mut self, worker: &str, f: F) -> Option<T>
    where
        F: FnOnce(&mut Worker) -> T,
    {
        self.workers_lut
            .get(worker)
            .map(|t| f(self.state.mutate().get_mut(t)))
    }

    /// Get the [Proxy] for the worker with the given name
    pub fn get_worker_proxy(&self, worker: &str) -> Option<Proxy<Worker>> {
        self.workers_lut.get(worker).copied()
    }

    /// Execute a function on every worker
    pub fn with_workers<F>(&self, mut f: F)
    where
        F: FnMut(&Worker),
    {
        for d in self.state.access().get_iter() {
            f(d)
        }
    }

    /// Execute a function on every mutable worker
    pub fn with_workers_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut Worker),
    {
        for d in self.state.mutate().get_iter_mut() {
            f(d)
        }
    }

    pub fn add_device<H, D, T, T1>(
        &mut self,
        hostname: H,
        device_type: D,
        tags: T,
    ) -> Option<String>
    where
        D: AsRef<str>,
        H: ToString,
        T: AsRef<[T1]>,
        T1: AsRef<str>,
    {
        let device_type = self.device_types_lut.get(device_type.as_ref())?;

        let tags = tags
            .as_ref()
            .iter()
            .filter_map(|t| self.tags_lut.get(t.as_ref()))
            .copied()
            .collect();

        let dev = {
            let m = self.state.mutate();

            let (dev, mut m) = self.devices.generate(m);

            let device = m.get_mut(&dev);
            device.hostname = hostname.to_string();
            device.tags = tags;
            device.device_type = *device_type;

            dev
        };

        self.devices_lut.insert(hostname.to_string(), dev);
        Some(self.state.access().get(&dev).hostname.clone())
    }

    pub fn add_device_type<D>(&mut self, name: D) -> String
    where
        D: ToString,
    {
        let dt = {
            let m = self.state.mutate();
            let (dt, mut m) = self.device_types.generate(m);

            let device_type = m.get_mut(&dt);
            device_type.name = name.to_string();
            dt
        };

        self.device_types_lut.insert(name.to_string(), dt);
        self.state.access().get(&dt).name.to_string()
    }

    pub fn add_job<D, T, T1>(&mut self, requested_device_type: Option<D>, tags: T) -> i64
    where
        D: AsRef<str>,
        T: AsRef<[T1]>,
        T1: AsRef<str>,
    {
        let j = {
            let m = self.state.mutate();

            let (j, mut m) = self.jobs.generate(m);

            let job = m.get_mut(&j);
            job.requested_device_type = requested_device_type
                .as_ref()
                .and_then(|dt| self.device_types_lut.get(dt.as_ref()))
                .copied();
            job.submit_time = Some(self.clock.now());
            job.tags = tags
                .as_ref()
                .iter()
                .filter_map(|t| self.tags_lut.get(t.as_ref()))
                .copied()
                .collect();

            j
        };

        let id = self.state.access().get(&j).id;
        self.jobs_lut.insert(id, j);
        id
    }

    pub fn schedule_job(&mut self, job: i64, device: &str) {
        let mut m = self.state.mutate();
        let j = m.get_mut(self.jobs_lut.get(&job).expect("invalid job id"));
        let d = self.devices_lut.get(device);
        j.actual_device = d.copied();
        j.state = JobState::Scheduled;
    }

    pub fn start_job(&mut self, job: i64) {
        let mut m = self.state.mutate();
        let j = m.get_mut(self.jobs_lut.get(&job).expect("invalid job id"));
        j.state = JobState::Running;
        j.start_time = Some(self.clock.now());
    }

    pub fn end_job(&mut self, job: i64, health: JobHealth) {
        let mut m = self.state.mutate();
        let j = m.get_mut(self.jobs_lut.get(&job).expect("invalid job id"));
        j.state = JobState::Finished;
        j.end_time = Some(self.clock.now());
        j.health = health;
    }

    pub fn add_tag<N>(&mut self, name: N) -> String
    where
        N: ToString,
    {
        let tag = {
            let m = self.state.mutate();
            let (t, mut m) = self.tags.generate(m);

            let tag = m.get_mut(&t);
            tag.name = name.to_string();

            t
        };

        self.tags_lut.insert(name.to_string(), tag);

        name.to_string()
    }

    pub fn add_worker<H>(&mut self, hostname: H) -> String
    where
        H: ToString,
    {
        let w = {
            let m = self.state.mutate();
            let (w, mut m) = self.workers.generate(m);

            let worker = m.get_mut(&w);
            worker.hostname = hostname.to_string();

            w
        };

        self.workers_lut.insert(hostname.to_string(), w);

        self.state.access().get(&w).hostname.to_string()
    }

    //// Add bulk devices
    pub fn generate_devices(&mut self, count: usize) -> Vec<String> {
        let mut devices = Vec::new();
        for v in self.bulk_devices.take_n(self.state.mutate(), count) {
            devices.push(v);
        }

        let a = self.state.access();
        devices
            .into_iter()
            .map(|d| {
                let hostname = &a.get(&d).hostname;
                self.devices_lut.insert(hostname.clone(), d);
                hostname.clone()
            })
            .collect()
    }

    pub fn generate_device_types(&mut self, count: usize) -> Vec<String> {
        let mut device_types = Vec::new();
        for v in self.bulk_device_types.take_n(self.state.mutate(), count) {
            device_types.push(v);
        }

        let a = self.state.access();
        device_types
            .into_iter()
            .map(|dt| {
                let name = &a.get(&dt).name;
                self.device_types_lut.insert(name.clone(), dt);
                name.clone()
            })
            .collect()
    }

    pub fn generate_jobs(&mut self, count: usize) -> Vec<i64> {
        let mut jobs = Vec::new();
        for v in self.bulk_jobs.take_n(self.state.mutate(), count) {
            jobs.push(v);
        }

        let a = self.state.access();
        jobs.into_iter()
            .map(|j| {
                let id = a.get(&j).id;
                self.jobs_lut.insert(id, j);
                id
            })
            .collect()
    }

    pub fn generate_tags(&mut self, count: usize) -> Vec<String> {
        let mut tags = Vec::new();
        for v in self.bulk_tags.take_n(self.state.mutate(), count) {
            tags.push(v);
        }

        let a = self.state.access();
        tags.into_iter()
            .map(|t| {
                let name = &a.get(&t).name;
                self.tags_lut.insert(name.clone(), t);
                name.clone()
            })
            .collect()
    }

    pub fn generate_workers(&mut self, count: usize) -> Vec<String> {
        let mut workers = Vec::new();
        for v in self.bulk_workers.take_n(self.state.mutate(), count) {
            workers.push(v);
        }

        let a = self.state.access();
        workers
            .into_iter()
            .map(|w| {
                let hostname = &a.get(&w).hostname;
                self.workers_lut.insert(hostname.clone(), w);
                hostname.clone()
            })
            .collect()
    }
}
