//! This crate provides a set of types for constructing mock servers
//! providing the [LAVA](https://docs.lavasoftware.org/lava/) REST API
//! with generated data.
//!
//! # Overview
//!
//! The main types in a Lava database have types in this crate:
//! - [`Alias`]
//! - [`Architecture`]
//! - [`BitWidth`]
//! - [`Core`]
//! - [`Device`]
//! - [`DeviceType`]
//! - [`Group`]
//! - [`Job`]
//! - [`ProcessorFamily`]
//! - [`Tag`]
//! - [`TestCase`]
//! - [`TestSet`]
//! - [`TestSuite`]
//! - [`User`]
//! - [`Worker`]
//!
//! There is a container type [`State`] which implements
//! [`Context`](persian_rug::Context) from the
//! [`persian-rug`](persian_rug) crate. All types are
//! [`GeneratableWithPersianRug`](boulder::GeneratableWithPersianRug)
//! and [`BuildableWithPersianRug`](boulder::BuildableWithPersianRug)
//! which are from the [`boulder`] crate.
//!
//! # LavaMock
//!
//! Most users will want to base their tests around [`LavaMock`],
//! which is a [`django-query`](django_query) derived server, which
//! provides all of the v0.2 query REST endpoints of a standard Lava
//! server. See the documentation for details of its limitations. The
//! data it serves comes from a [`SharedState`] (a synchronised
//! wrapper over a [`State`]) which can both be populated with default
//! data as a starting point, and also updated on the fly to simulate
//! whatever update pattern is desired.
//!
//! Example:
//! ```rust
//! use futures::stream::TryStreamExt;
//! use lava_api_mock::{LavaMock, PaginationLimits, PopulationParams, SharedState};
//! use lava_api::Lava;
//!
//! # tokio_test::block_on( async {
//! // Make the mock server
//! let limits = PaginationLimits::new();
//! let population = PopulationParams::new();
//! let mock = LavaMock::new(SharedState::new_populated(population), limits).await;
//!
//! // Make the Lava client for reading back data from the server
//! let lava = Lava::new(&mock.uri(), None).expect("failed to make lava client");
//!
//! // Read back the devices using the Lava client
//! let mut ld = lava.devices();
//! while let Some(device) = ld
//!     .try_next()
//!     .await
//!     .expect("failed to read devices from server")
//! {
//!     println!("Got device {:?}", device);
//! }
//! # });
//! ```

mod devices;
mod devicetypes;
mod jobs;
mod junit;
mod lava_mock;
mod state;
mod tags;
mod testcases;
mod users;
mod workers;

pub use devices::{Device, Health as DeviceHealth, State as DeviceState};
pub use devicetypes::{Alias, Architecture, BitWidth, Core, DeviceType, ProcessorFamily};
pub use jobs::Job;
pub use jobs::{Health as JobHealth, State as JobState};
pub use junit::{junit_endpoint, JunitEndpoint};
pub use lava_mock::{LavaMock, PaginationLimits};
pub use state::{PopulationParams, SharedState, State};
pub use tags::Tag;
pub use testcases::{Metadata, PassFail, TestCase, TestSet, TestSuite};
pub use users::{Group, User};
pub use workers::Worker;
