// SPDX-License-Identifier: MPL-2.0
//
// Part of Auguth Labs open-source softwares.
// Built for the Substrate framework.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 Auguth Labs (OPC) Pvt Ltd, India

#![cfg_attr(not(feature = "std"), no_std)]
// ===============================================================================
// ````````````````````````````````` FRAME SUITE `````````````````````````````````
// ===============================================================================

//! A composable, type-driven foundation for building modular runtime systems
//! on top of Substrate.
//!
//! This crate is a collection of **orthogonal, interoperable abstractions**
//! that capture recurring runtime patterns and execution without prescribing
//! concrete implementations.
//!
//! The focus is not on providing ready-made systems, but on defining
//! **reusable semantics and capabilities** that can be composed, extended,
//! and adapted across different contexts.
//!
//! The design is guided by:
//!
//! - **Type-first abstractions**
//! - **Plugin-driven behavior**
//! - **Decoupled structure, storage, and execution**
//!
//! The crate is expected to evolve over time, growing as new reusable patterns
//! emerge, while maintaining a consistent approach to abstraction and composition.
//!
//! ## How Everything is Designed
//!
//! The crate follows a consistent design model across all modules:
//!
//! - **Traits define semantics**
//!   Each module exposes traits that describe *what a system means*,
//!   not how it is implemented.
//!
//! - **Types encode constraints**
//!   Capabilities, invariants, and relationships are expressed through
//!   associated types and bounds, ensuring correctness at compile time.
//!
//! - **Behavior is externalized**
//!   Logic is not embedded into core structures. Instead, it is provided
//!   through pluggable models and context-driven execution.
//!
//! - **Storage is not assumed**
//!   Data layout and persistence are left to the implementing context,
//!   allowing the same abstraction to work across different storage strategies.
//!
//! - **Composition is the mechanism**
//!   Larger systems emerge by combining small, orthogonal primitives,
//!   rather than extending monolithic components.
//!
//! This results in abstractions that are:
//! - reusable across domains
//! - extensible without modification
//! - adaptable to different runtime environments
//!
//! ## Architecture Overview
//!
//! The crate is organized into domain-oriented modules, each contributing
//! a focused set of traits and primitives. These modules are intentionally
//! **loosely coupled** and can be used independently or composed together
//! to form higher-level systems.
//!
//! The crate is organized from
//!
//! ```text
//! low-level primitives -> structural abstractions -> domain systems -> execution
//! ```
//!
//! ### Core Primitives
//!
//! - [`base`]         : Foundational traits for deterministic, codec-safe types  
//! - [`fixedpoint`]   : Deterministic numeric and fixed-point abstractions  
//! - [`keys`]         : Deterministic identifier derivation  
//! - [`mutation`]     : Mutation-centric abstractions over ownership  
//! - [`misc`]         : Small, reusable building blocks across semantic-boundaries
//!
//! ### Structural & Behavioral Core
//!
//! - [`virtuals`]     : Type-driven virtual struct system (decoupled structure layer)  
//! - [`plugins`]      : Pluggable, type-safe execution and behavior layer  
//!
//! ### Progression & Accumulation
//!
//! - [`accumulators`] : Step-based progression and accumulation models  
//! - [`xp`]           : Experience points as a pseudo-fungible progression primitive  
//!
//! ### Value & Financial Semantics
//!
//! - [`assets`]       : Lazy, receipt-based accounting models  
//! - [`commitment`]   : Value bonding and intent-driven primitives  
//!
//! ### Coordination & Governance
//!
//! - [`roles`]        : Role lifecycle, funding, and incentive semantics  
//! - [`elections`]    : Generic, plugin-based selection and weighting systems  
//! - [`blockchain`]   : Author lifecycle, rewards, and coordination abstractions  
//!
//! ### Execution Layer
//!
//! - [`routines`]     : Structured Best-effort offchain-workers execution model  
//!
//! ## Design Principles
//!
//! ### Decoupled Concerns
//!
//! Structure, storage, and behavior are treated as independent dimensions:
//!
//! - **Structure**: expressed via traits and type-level schemas  
//! - **Storage**: abstracted or externalized  
//! - **Behavior**: injected via pluggable models  
//!
//! This separation allows systems to evolve without forcing redesigns
//! across unrelated layers.
//!
//! ### Plugin-Driven Execution
//!
//! Behavior is modeled as **pluggable units of computation**, enabling:
//!
//! - compile-time selection of logic  
//! - context-driven customization  
//! - interchangeable implementations without changing interfaces  
//!
//! ### Type-Level Composition
//!
//! The framework encodes meaning through types:
//!
//! - capabilities and constraints live in trait bounds  
//! - composition emerges from generic relationships  
//! - correctness is enforced at compile time  
//!
//! ### Lazy & Deferred Semantics
//!
//! Several modules adopt **lazy or deferred evaluation models** to:
//!
//! - minimize unnecessary state updates  
//! - defer computation until it is required  
//! - scale efficiently with system complexity  
//!
//! ## Hygiene
//!
//! All public symbols are **re-exported at the crate root**.
//!
//! This ensures:
//!
//! - a flat and ergonomic import surface  
//! - no need to depend on internal module paths  
//! - consistent and predictable naming across the crate  

// ===============================================================================
// ``````````````````````````````````` MODULES ```````````````````````````````````
// ===============================================================================

pub mod accumulators;
pub mod assets;
pub mod base;
pub mod blockchain;
pub mod commitment;
pub mod elections;
pub mod fixedpoint;
pub mod forks;
pub mod keys;
pub mod misc;
pub mod mutation;
pub mod plugins;
pub mod roles;
pub mod routines;
pub mod virtuals;
pub mod xp;

// ===============================================================================
// `````````````````````````````````` RE-EXPORTS `````````````````````````````````
// ===============================================================================

pub use accumulators::*;
pub use assets::*;
pub use base::*;
pub use blockchain::*;
pub use commitment::*;
pub use elections::*;
pub use fixedpoint::*;
pub use forks::*;
pub use keys::*;
pub use misc::*;
pub use mutation::*;
pub use plugins::*;
pub use roles::*;
pub use routines::*;
pub use virtuals::*;
pub use xp::*;
