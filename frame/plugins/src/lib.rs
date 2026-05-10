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
// ````````````````````````````````` FRAME PLUGINS `````````````````````````````````
// ===============================================================================

//! A plugin registry system built on top of [`frame_suite`], providing
//! concrete, reusable implementations of plugin-driven behavior.
//!
//! This crate does not define new abstractions. Instead, it **realizes**
//! the semantics defined in `frame_suite` by supplying **ready-to-use
//! plugin models and families** that can be anchored into Substrate runtime systems.
//!
//! Where `frame_suite` defines *what is possible*, this crate provides
//! examples of *how those possibilities can be instantiated*.
//!
//! ## Design
//!
//! The crate follows the same design principles as `frame_suite` and is
//! implemented using the [`frame_suite::plugins`] macro system:
//!
//! - **Plugins are units of behavior**
//!   Each model represents a single, well-defined transformation or operation,
//!   defined via [`plugin_model!`](frame_suite::plugin_model).
//!
//! - **Families compose behavior**
//!   Related operations are grouped into plugin families using
//!   [`define_family!`](frame_suite::define_family), forming cohesive execution
//!   surfaces for higher-level systems.
//!
//! - **Context drives execution**
//!   Models may depend on external configuration or environment via context,
//!   enabling flexible and runtime-specific behavior.
//!
//! - **No assumptions beyond the contract**
//!   All implementations adhere strictly to the trait contracts defined in
//!   `frame_suite`, without introducing hidden coupling.
//!
//! ## Module Overview
//!
//! The crate is organized into **domain categories of plugin sets**, each targeting
//! a specific class of abstractions from `frame_suite`.
//!
//! ### Value & Accounting
//!
//! - [`balances`]   : Lazy balance plugin families and models  
//!
//! ### Coordination & Selection
//!
//! - [`elections`]  : Election algorithms (flat, fair, and beyond)  
//!
//! ### Influence & Weighting
//!
//! - [`influence`]  : Influence (Quantifiable Power) transformation models  
//!
//! ### Rewards & Distribution
//!
//! - [`rewards`]    : Reward computation (`payout`) and distribution (`payee`) models  
//!
//! ### Penalty & Normalization
//!
//! - [`penalty`]    : Penalty normalization and bounding models  
//!
//! ## Design Intent
//!
//! This crate acts as a **behavior layer** over `frame_suite`:
//!
//! - it demonstrates how abstractions can be implemented  
//! - it provides reusable building blocks for common patterns  
//! - it avoids locking systems into a single interpretation  
//!
//! New models can be added freely as long as they:
//!
//! - respect the underlying trait contracts  
//! - remain composable and independent  
//! - do not introduce unnecessary coupling  
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

pub mod balances;
pub mod elections;
pub mod influence;
pub mod penalty;
pub mod rewards;

// ===============================================================================
// `````````````````````````````````` RE-EXPORTS `````````````````````````````````
// ===============================================================================

pub use balances::*;
pub use elections::*;
pub use influence::*;
pub use penalty::*;
pub use rewards::*;
