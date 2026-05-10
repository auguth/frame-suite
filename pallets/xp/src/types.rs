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

// ===============================================================================
// ``````````````````````````````````` XP TYPES ``````````````````````````````````
// ===============================================================================

//! Core types and aliases for the XP system.
//!
//! This module defines the primary structures and type aliases used by
//! [`pallet_xp`](crate). These types are publicly exposed and used across
//! the pallet's APIs for representing XP-related data.
//!
//! Trait implementations provided by this crate's [`Pallet`] can use these types
//! via trait-bound equality constraints to ensure type alignment with this pallet's
//! concrete implementations if neccessary.
//!
//! ## Example
//!
//! ```ignore
//! mod pallet {
//!     use pallet_xp::types::Xp as XpData;
//!
//!     pub trait Config<I: 'static>: frame_system::Config {
//!         type XpAdapter: XpSystem<Xp = XpData<Self, I>>;
//!     }
//! }
//! ```

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- Local crate imports ---
use crate::{Config, InitXp, Pallet};

// --- FRAME System ---
use frame_system::{pallet, pallet_prelude::BlockNumberFor};

// --- FRAME Suite ---
use frame_suite::xp::{XpLock, XpReserve, XpSystem};

// --- Substrate primitives ---
use sp_core::{Decode, Encode, MaxEncodedLen};
use sp_runtime::{traits::Zero, RuntimeDebug};

use codec::DecodeWithMemTracking;
use scale_info::TypeInfo;

// --- Derive crates ---
use derive_more::Constructor;

// --- Scale-codec crates ---
use serde::{Deserialize, Serialize};

// ===============================================================================
// ``````````````````````````````````` ALIASES ```````````````````````````````````
// ===============================================================================

/// XP account identifier.
pub type XpId<T> = <T as pallet::Config>::AccountId;

/// Reason identifier used when reserving XP points.
pub type ReserveReason<T, I> = <Pallet<T, I> as XpReserve>::ReserveReason;

/// Reason identifier used when locking XP points.
pub type LockReason<T, I> = <Pallet<T, I> as XpLock>::LockReason;

/// Scalar XP value type representing the numerical XP amount.
pub type XpValue<T, I> = <Pallet<T, I> as XpSystem>::Points;

// ===============================================================================
// ``````````````````````````````````` STRUCTS ```````````````````````````````````
// ===============================================================================

/// The main XP data structure that is utilized on implementation of [`XpSystem::Xp`].
///
/// It provides a high-level detail for managing liquid points,
/// reserved points, locked points, and reputation-based pulse tracking with
/// timestamp information.
///
/// ### Point Categories
/// - **Free Points**: Liquid XP that the owner can freely access and use.
/// - **Reserved Points**: XP temporarily set aside for Runtime specific purposes.
/// - **Locked Points**: XP that is restricted by the Runtime provider (other pallets)
///   or implementor.
///
/// ### Reputation System
/// - **Pulse**: A discrete accumulator that tracks XP mutation frequency for reputation.
/// - **Timestamp**: Block number of the last XP increment for heartbeat tracking.
#[derive(Encode, Decode, Copy, MaxEncodedLen, TypeInfo, DecodeWithMemTracking)]
#[scale_info(skip_type_params(T, I))]
pub struct Xp<T: Config<I>, I: 'static> {
    /// Liquid XP points that the owner can freely access.
    pub free: T::Xp,

    /// Reserved XP points that are temporarily set aside for specific purposes.
    ///
    /// This field aggregates all reserved XP across different `RuntimeReason`s,
    /// enabling efficient access to the total reserved balance without needing
    /// to iterate through individual reservation records.
    pub reserve: T::Xp,

    /// Locked XP points that are restricted by the Runtime provider or implementor.
    ///
    /// This field aggregates all locked XP across different `RuntimeReason`s,
    /// enabling efficient access to the total locked balance without needing
    /// to iterate through individual lock records.
    pub lock: T::Xp,

    /// Reputation-based pulse accumulator that tracks XP mutation frequency.
    ///
    /// The provider represents the runtime intent, awarding XP based on
    /// completed work.
    ///
    /// The pulse acts as a "heartbeat" of XP activity, serving as a reputational
    /// metric for each XP account. It is proportional to the frequency and amount
    /// of XP increments, and may influence the raw XP awarded for future actions.
    ///
    /// In an untrusted environment, `pulse` is used as a reputational resource,
    /// allowing the system to adjust raw XP based on the quality and consistency
    /// of the account's activity as reflected by its pulse.
    pub pulse: Accumulator<T, I>,

    /// The block number at which XP was last incremented.
    ///
    /// This timestamp is used to identify inactive ("dead") XP accounts and can be
    /// leveraged to conditionally determine whether to increase reputation (i.e., pulse)
    /// based on recent activity, or if the XP is subjected to reaping procedures.
    pub timestamp: BlockNumberFor<T>,
}

/// A data structure that associates XP points with a specific reason identifier.
///
/// This struct is used to track XP points that are allocated for specific purposes,
/// such as locks or reserves. Each `IdXp` instance represents a portion of XP points
/// that are tied to a particular reason, enabling granular tracking and management
/// of XP allocations.
#[derive(
    Encode,
    Decode,
    Clone,
    PartialEq,
    Eq,
    Copy,
    RuntimeDebug,
    MaxEncodedLen,
    TypeInfo,
    Constructor,
    DecodeWithMemTracking,
)]
#[scale_info(skip_type_params(T, I))]
pub struct IdXp<Id, Value> {
    /// The reason identifier that categorizes the purpose of these XP points.
    ///
    /// This field uses the runtime's reason system to provide type-safe
    /// categorization of XP allocations.
    pub id: Id,

    /// The amount of XP points associated with this reason.
    ///
    /// Represents the actual XP points that has been allocated for the
    /// specific purpose identified by the `id` field.
    pub points: Value,
}

/// Internal accumulator structure for discrete XP pulse tracking.
///
/// This struct implements the accumulator pattern for reputation-based XP systems,
/// where XP activities are tracked through discrete steps that accumulate towards
/// threshold-based value increments. It serves as the core data structure for the
/// pulse reputation system.
///
/// ## Fields
/// - `value` - The current accumulated XP value (reputation level)
/// - `step` - The current step progress towards the next value increment
#[derive(Encode, Decode, Copy, MaxEncodedLen, TypeInfo, DecodeWithMemTracking)]
#[scale_info(skip_type_params(T, I))]
pub struct Accumulator<T: Config<I>, I: 'static> {
    /// The current accumulated value representing the reputation level.
    ///
    /// This field holds the meaningful accumulated result that represents
    /// the user's reputation/heartbeat or XP level. It increments when step
    /// thresholds are reached through the discrete accumulation process.
    pub value: T::Pulse,

    /// The current step progress towards the next value increment.
    ///
    /// This field tracks intermediate fractional progress between value increments.
    ///
    /// Steps accumulate until they reach a threshold defined by the stepper,
    /// at which point the value is incremented and steps are reset or adjusted.
    pub step: T::Pulse,
}

/// Configuration structure for discrete accumulation operations.
///
/// This struct defines the operational parameters for discrete accumulation,
/// working in conjunction with the [`Accumulator`] to implement threshold-based
/// progression systems. It encapsulates the rules that govern how steps are
/// applied and when accumulated values should be incremented.
///
/// ##  Fields
/// - `threshold` - The step count required to increment the accumulated value
/// - `per_count` - The number of steps added per accumulation operation
#[derive(
    Encode, Decode, MaxEncodedLen, TypeInfo, Serialize, Deserialize, DecodeWithMemTracking,
)]
#[scale_info(skip_type_params(T, I))]
pub struct Stepper<T: Config<I>, I: 'static> {
    /// The step count threshold required to increment the accumulated value.
    ///
    /// When the accumulator's step count reaches or exceeds this threshold,
    /// the accumulated value is incremented and the step count is adjusted.
    /// This defines the "cost" of each value increment in terms of steps.
    pub threshold: T::Pulse,

    /// The number of steps added per accumulation operation.
    ///
    /// This defines how much progress is made towards the threshold with each
    /// discrete accumulation operation. Multiple operations may be required
    /// to reach the threshold and trigger a value increment.
    pub per_count: T::Pulse,
}

/// Genesis configuration entry for an XP identity.
///
/// Used within [`crate::GenesisConfig`] to initialize XP identities
/// and assign their owners at chain genesis via `new_xp`.
///
/// This serves only to populate XP identities (keys) and establish
/// ownership. It does **not** allocate or assign any XP points.
///
/// The initialization mechanism is not compatible with fungible balance
/// systems. Once created, XP points can be populated later through
/// `earn_xp` or other compatible fungible interfaces.
#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    RuntimeDebug,
    Clone,
    PartialEq,
    Eq,
    MaxEncodedLen,
    TypeInfo,
    Deserialize,
    Serialize,
)]
pub struct GenesisAcc<Owner, Id> {
    /// Owner of the XP identity.
    pub owner: Owner,

    /// Identifier of the XP identity.
    pub id: Id,
}

/// Enumerates configurable XP parameters that may be forcibly overridden
/// at runtime through privileged (root/governance) operations.
#[derive(Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo)]
#[scale_info(skip_type_params(T, I))]
pub enum ForceGenesisConfig<T: Config<I>, I: 'static> {
    /// Update minimum pulse-counts required to become reputed.
    MinPulse(T::Pulse),
    /// Update initial XP granted at creation.
    InitXp(T::Xp),
    /// Update pulse accumulation parameters.
    PulseFactor {
        /// Threshold required to increment pulse value.
        threshold: T::Pulse,
        /// Step increment applied per earn call.
        per_count: T::Pulse,
    },
    // Update minimum timestamp (block number) for XP liveness.
    MinTimeStamp(BlockNumberFor<T>),
}

/// Tracks an identity's progress toward earning XP.
///
/// An identity must complete a number of `earn_xp` actions before XP
/// starts being counted. Until then, progress is tracked but not rewarded.
///
/// Note: Actions are counted per block. Multiple `earn_xp` calls within
/// the same block are treated as a single action.
#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    RuntimeDebug,
    Clone,
    PartialEq,
    Eq,
    MaxEncodedLen,
    TypeInfo,
)]
#[scale_info(skip_type_params(T, I))]
pub enum XpEligibility<T: Config<I>, I: 'static> {
    /// XP is not yet being counted.
    Progressing(
        /// Remaining blocks with valid `earn_xp` actions required
        /// before XP starts being counted.
        T::Pulse,
    ),

    /// XP is now active and will be counted.
    Earning,
}

/// Represents the progression mechanics behind XP scaling.
///
/// Exposes the current level, progress toward the next increment,
/// and the parameters that control how progress is accumulated.
#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    RuntimeDebug,
    Clone,
    PartialEq,
    Eq,
    MaxEncodedLen,
    TypeInfo,
)]
pub struct XpProgress<T: Config<I>, I: 'static> {
    /// Current multiplier level.
    pub level: T::Pulse,

    /// Progress toward the next level.
    pub progress: T::Pulse,

    /// Total progress required to reach the next level.
    pub threshold: T::Pulse,

    /// Progress gained per `earn_xp` action.
    pub per_action: T::Pulse,
}

/// Snapshot of an identity's XP-related state.
///
/// Combines balance information with XP activation and multiplier data.
/// Designed for RPC responses and UI consumption, where both current value
/// and progression state need to be displayed together.
#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    RuntimeDebug,
    Clone,
    PartialEq,
    Eq,
    MaxEncodedLen,
    TypeInfo,
)]
#[scale_info(skip_type_params(T, I))]
pub struct XpState<T: Config<I>, I: 'static> {
    /// Freely usable balance.
    pub liquid: T::Xp,

    /// Balance reserved for protocol-level usage.
    pub reserved: T::Xp,

    /// Balance locked by constraints (e.g. vesting, staking).
    pub locked: T::Xp,

    /// Current XP multiplier.
    ///
    /// Returns `1` while XP is not yet active. Once eligible,
    /// this reflects the effective multiplier applied to XP gains.
    ///
    /// Note:
    /// The multiplier is applied only if the next `earn_xp` call occurs
    /// in a new block (`last_earn < current block`). If multiple calls
    /// are made within the same block, only the first applies the multiplier;
    /// subsequent calls are unscaled.
    pub multiplier: T::Pulse,

    /// XP activation state.
    ///
    /// Indicates whether XP is currently being earned, or how much
    /// progress remains before it starts counting.
    pub eligibility: XpEligibility<T, I>,
}

// ===============================================================================
// ```````````````````````````````` INHERENT IMPLS ```````````````````````````````
// ===============================================================================

impl<T: Config<I>, I: 'static> Stepper<T, I> {
    /// Creates and returns a new stepper instance with the specified threshold
    /// and per-count values.
    ///
    /// **Condition**
    ///
    /// Returns `None` if `per_count >= threshold`, as this would cause immediate
    /// value increments without meaningful step accumulation.
    pub fn new(threshold: T::Pulse, per_count: T::Pulse) -> Option<Self> {
        if per_count > threshold {
            return None;
        }
        Some(Self {
            threshold,
            per_count,
        })
    }
}

// ===============================================================================
// ````````````````````````````````` DERIVE IMPLS ````````````````````````````````
// ===============================================================================

// Manual impls since derive macros cannot handle the `I` instance generic
// without introducing unnecessary trait bounds.

impl<T: Config<I>, I: 'static> Clone for Xp<T, I> {
    fn clone(&self) -> Self {
        Self {
            free: self.free,
            reserve: self.reserve,
            lock: self.lock,
            pulse: self.pulse.clone(),
            timestamp: self.timestamp,
        }
    }
}

impl<T: Config<I>, I: 'static> Default for Xp<T, I> {
    fn default() -> Self {
        Self {
            // Pallet provides StorageValue for new Xp's beginning liquidity for
            // rewarding participation (not a constant)
            free: InitXp::<T, I>::get(),
            // Accumulator is set to default - marks beginning.
            pulse: Default::default(),
            // No reserved points, zero value on initialization of Xp
            reserve: T::Xp::zero(),
            // No locked points, zero value on initialization of Xp
            lock: T::Xp::zero(),
            // Timestamp is set to current runtime block number on XP initialization
            timestamp: frame_system::Pallet::<T>::block_number(),
        }
    }
}

impl<T: Config<I>, I: 'static> PartialEq for Xp<T, I> {
    fn eq(&self, other: &Self) -> bool {
        self.free == other.free
            && self.reserve == other.reserve
            && self.lock == other.lock
            && self.pulse == other.pulse
            && self.timestamp == other.timestamp
    }
}

impl<T: Config<I>, I: 'static> Eq for Xp<T, I> {}

impl<T: Config<I>, I: 'static> core::fmt::Debug for Xp<T, I> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Xp")
            .field("free", &self.free)
            .field("reserve", &self.reserve)
            .field("lock", &self.lock)
            .field("pulse", &self.pulse)
            .field("timestamp", &self.timestamp)
            .finish()
    }
}

impl<T: Config<I>, I: 'static> Clone for Accumulator<T, I> {
    fn clone(&self) -> Self {
        Self {
            value: self.value,
            step: self.step,
        }
    }
}

impl<T: Config<I>, I: 'static> Default for Accumulator<T, I> {
    /// Creates a new accumulator with both value and step initialized to their
    /// default values (zero).
    fn default() -> Self {
        Self {
            value: Default::default(),
            step: Default::default(),
        }
    }
}

impl<T: Config<I>, I: 'static> PartialEq for Accumulator<T, I> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value && self.step == other.step
    }
}

impl<T: Config<I>, I: 'static> Eq for Accumulator<T, I> {}

impl<T: Config<I>, I: 'static> core::fmt::Debug for Accumulator<T, I> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Accumulator")
            .field("value", &self.value)
            .field("step", &self.step)
            .finish()
    }
}

/// This is only utilized for mock testing/instances. Elsewhere
/// [`Stepper::new`] should be utilized
///
/// A Mock Default implemetation for the Stepper struct.
///
/// This is confidently not utilized for runtime, and only as a marker
/// for `StorageValue`'s `ValueQuery` default trait bound satisfaction since
/// genesis config already requires this struct for `PulseFactor`.
impl<T: Config<I>, I: 'static> Default for Stepper<T, I> {
    fn default() -> Self {
        Stepper::<T, I>::new(50u8.into(), 10u8.into()).unwrap()
    }
}

impl<T, I> Clone for Stepper<T, I>
where
    T: Config<I>,
    T::Pulse: Clone,
{
    fn clone(&self) -> Self {
        Self {
            threshold: self.threshold,
            per_count: self.per_count,
        }
    }
}

impl<T, I> PartialEq for Stepper<T, I>
where
    T: Config<I>,
    T::Pulse: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.threshold == other.threshold && self.per_count == other.per_count
    }
}

impl<T, I> Eq for Stepper<T, I>
where
    T: Config<I>,
    T::Pulse: Eq,
{
}

use core::fmt;

impl<T, I> fmt::Debug for Stepper<T, I>
where
    T: Config<I>,
    T::Pulse: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Stepper")
            .field("threshold", &self.threshold)
            .field("per_count", &self.per_count)
            .finish()
    }
}

impl<T: Config<I>, I: 'static> Clone for ForceGenesisConfig<T, I> {
    fn clone(&self) -> Self {
        match self {
            Self::MinPulse(v) => Self::MinPulse(*v),
            Self::InitXp(v) => Self::InitXp(*v),
            Self::PulseFactor {
                threshold,
                per_count,
            } => Self::PulseFactor {
                threshold: *threshold,
                per_count: *per_count,
            },
            Self::MinTimeStamp(v) => Self::MinTimeStamp(*v),
        }
    }
}

impl<T: Config<I>, I: 'static> core::fmt::Debug for ForceGenesisConfig<T, I> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MinPulse(v) => f.debug_tuple("MinPulse").field(v).finish(),
            Self::InitXp(v) => f.debug_tuple("InitXp").field(v).finish(),
            Self::PulseFactor {
                threshold,
                per_count,
            } => f
                .debug_struct("PulseFactor")
                .field("threshold", threshold)
                .field("per_count", per_count)
                .finish(),
            Self::MinTimeStamp(v) => f.debug_tuple("MinTimeStamp").field(v).finish(),
        }
    }
}

impl<T: Config<I>, I: 'static> PartialEq for ForceGenesisConfig<T, I>
where
    T::Pulse: PartialEq,
    T::Xp: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::MinPulse(a), Self::MinPulse(b)) => a == b,
            (Self::InitXp(a), Self::InitXp(b)) => a == b,
            (
                Self::PulseFactor {
                    threshold: a_t,
                    per_count: a_p,
                },
                Self::PulseFactor {
                    threshold: b_t,
                    per_count: b_p,
                },
            ) => a_t == b_t && a_p == b_p,
            (Self::MinTimeStamp(a), Self::MinTimeStamp(b)) => a == b,
            _ => false,
        }
    }
}

impl<T: Config<I>, I: 'static> Eq for ForceGenesisConfig<T, I>
where
    T::Pulse: Eq,
    T::Xp: Eq,
{
}

// ===============================================================================
// `````````````````````````````````` UNIT TESTS `````````````````````````````````
// ===============================================================================

#[cfg(test)]
/// Unit tests for [`crate::types`]
mod tests {

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````````` IMPORTS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    // -- Local Crate Imports --
    use crate::mock::*;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // `````````````````````````````````` UNIT TESTS `````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn xp_default_check() {
        xp_test_ext().execute_with(|| {
            System::set_block_number(2);
            let xp = MockXp::default();
            assert_eq!(xp.free, 10);
            assert_eq!(xp.lock, 0);
            assert_eq!(xp.reserve, 0);
            assert_eq!(xp.pulse.value, 0);
            assert_eq!(xp.timestamp, 2);
        });
    }

    #[test]
    fn stepper_new_success() {
        xp_test_ext().execute_with(|| {
            let stepper = Stepper::new(100, 10).unwrap();
            assert_eq!(stepper.threshold, 100);
            assert_eq!(stepper.per_count, 10);
        });
    }

    #[test]
    fn stepper_new_fail_none() {
        xp_test_ext().execute_with(|| {
            let threshold = 150;
            let per_count = 200;
            assert_eq!(Stepper::new(threshold, per_count), None);
        });
    }

    #[test]
    fn accumulator_default_check() {
        xp_test_ext().execute_with(|| {
            let accumulator = Accumulator::default();
            assert_eq!(accumulator.value, 0);
            assert_eq!(accumulator.step, 0);
        });
    }
}