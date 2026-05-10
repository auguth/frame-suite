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
// ````````````````````````````````` PALLET XP ```````````````````````````````````
// ===============================================================================

//! The XP pallet provides a modular and extensible system for managing
//! **Experience Points (XP)** as a non-monetary, programmable primitive
//! representing reputation, contribution, or progression.
//!
//! This pallet is built on top of [`frame_suite::xp`] and relies heavily
//! on its abstractions. It is strongly recommended to understand those traits
//! before using this pallet.
//!
//! ## Overview
//!
//! - [`Config`] - Runtime configuration
//! - [`Call`] - Dispatchable extrinsics
//! - [`Pallet`] - Trait implementation for external modules
//!
//! Unlike traditional fungible systems such as `pallet_balances`, XP is:
//! - **non-transferable as value**
//! - **not issuance-based** (no total supply tracking)
//! - **earned through controlled mechanisms**
//!
//! The only user-facing transfer is **ownership transfer** of an XP key via
//! [`Call::handover`]. All XP value changes must occur through
//! [`XpMutate::earn_xp`](frame_suite::xp::XpMutate::earn_xp)
//! (typically invoked by runtime logic or other pallets) or internal runtime
//! mechanisms.
//!
//! ## Identity
//!
//! XP is **key-based**, not account-based:
//!
//! - Each XP entry is identified by an [`XpId`](crate::types::XpId)
//! - Each XP key has exactly **one owner**
//! - A single account can own **multiple XP keys** ([`XpOwners`])
//!
//! ```text
//! Account -- owns --> XpId (key)
//!                  |- free XP
//!                  |- reserved XP
//!                  |- locked XP
//! ```
//!
//! XP keys do not hold private keys and therefore require explicit ownership.
//! Keys are deterministically generated using
//! [`XpOwner::xp_key_gen`](frame_suite::xp::XpOwner::xp_key_gen).
//!
//! ## Lifecycle
//!
//! The standard XP lifecycle is:
//!
//! ```text
//! begin_xp -> earn_xp -> (reserve / lock) -> reap
//! ```
//!
//! - Use [`BeginXp::begin_xp`](frame_suite::xp::BeginXp::begin_xp) for
//!   safe initialization
//! - Use [`XpMutate::earn_xp`](frame_suite::xp::XpMutate::earn_xp) to
//!   grant XP
//!
//! > Note: For pre-defined accounts, prefer initializing via [`GenesisConfig`]
//! > instead of [`BeginXp::begin_xp`](frame_suite::xp::BeginXp::begin_xp).
//!
//! XP earning is **not a simple increment**. It integrates a **pulse-based
//! reputation system** that:
//! - prevents same-block abuse
//! - enforces a minimum activity threshold ([`MinPulse`])
//! - scales rewards based on accumulated reputation
//! - optionally accelerates growth when XP is locked
//!
//! At a high level:
//! - Initially, actions **build reputation (pulse)** instead of granting XP
//! - Once active, XP grows approximately as: `XP += points * reputation`
//!
//! ```ignore
//! if pulse < MinPulse:
//!     build reputation only
//! else:
//!     XP += points * pulse
//! ```
//!
//! This results in:
//! - early usage -> builds reputation
//! - consistent usage -> earns increasing XP
//! - higher reputation -> amplifies future rewards
//!
//! ## Constraints: Reserve & Lock
//!
//! XP supports two constraint mechanisms:
//!
//! - [`XpReserve`](frame_suite::xp::XpReserve) - soft reservation
//!   (withdrawable, intent-based)
//! - [`XpLock`](frame_suite::xp::XpLock) - strict locking
//!   (non-partial withdrawal, protocol-enforced)
//!
//! These are accessible via XP traits directly, or through the fungible adapter
//! for interoperability.
//!
//! ## Fungible Compatibility
//!
//! The pallet provides partial implementations of
//! [`fungible`](frame_support::traits::fungible) unbalanced traits
//! to support interoperability with pallets expecting balance-like behavior,
//! allowing the same logic to operate across both XP and fungible systems
//! when used appropriately.
//!
//! However:
//! - XP is **not fungible**
//! - `total_issuance` and `active_issuance` are undefined
//! - transfers of value are disallowed
//!
//! Prefer using XP-specific traits for precise-requirements.
//!
//! ## Origin Model
//!
//! Most Substrate logic operates on account-based origins. In this system,
//! execution still originates from an account, but the **XP key acts as the
//! primary subject of state transitions** for XP-related operations.
//!
//! Runtime logic should treat the XP key as the unit of interaction and
//! authorization, rather than the account itself.
//!
//! ```ignore
//! origin: AccountId
//! input: XpId
//! ensure owner(origin, XpId)
//! execute on XpId
//! ```
//!
//! This is facilitated via [`Call::call`], where an XP key is provided and
//! validated against its owner, enabling XP-scoped execution within the
//! standard origin-driven model.
//!
//! ## Reaping & Liveness
//!
//! XP does not use existential deposits. Instead, liveness is determined via
//! activity:
//!
//! - Each XP entry tracks a timestamp updated on XP earning, indicating activity
//! - [`MinTimeStamp`] (set via root) defines the minimum liveness threshold
//! - If an XP's timestamp falls below this threshold, it is considered inactive
//! - XP with active locks is treated as in-use (runtime intent) and cannot be reaped
//! - Inactive XP entries can be **reaped** via [`Call::dispose`] and are
//!   permanently invalidated
//!
//! This ensures XP reflects active participation or active usage, rather than passive
//! holding.
//!
//! ## Listeners & Hooks
//!
//! The pallet exposes extensibility via [`Config::Extensions`], where the current
//! extensions are listener traits defined in [`frame_suite::xp`].
//!
//! Each XP lifecycle event (create, earn, slash, reserve, lock, reap, transfer)
//! invokes the corresponding listener hook, independent of standard event emission.
//!
//! - Listeners are always executed regardless of [`Config::EmitEvents`]
//! - Using XP traits directly is expected to provide accurate, intent-aligned hooks
//! - Using fungible adapters will still function, but may not fully reflect XP-specific
//!   semantics
//!
//! ## Genesis Configuration
//!
//! [`GenesisConfig`] sets how XP behaves from the start:
//!
//! - [`InitXp`]  
//!   Starting XP assigned when a new XP entry is created.
//!
//! - [`PulseFactor`]
//!   Controls how reputation (pulse) grows over time.  
//!   Repeated actions increase an internal counter, which periodically
//!   increases the pulse value.
//!     ```ignore
//!     step += per_count
//!     if step >= threshold:
//!         pulse += 1
//!         step resets
//!     ```
//!
//! - [`MinPulse`]  
//!   Minimum reputation required before XP is awarded.  
//!   Below this threshold, actions only build reputation.  
//!   Once reached, actions begin granting XP (scaled by reputation).
//!
//! - [`MinTimeStamp`]  
//!   Minimum activity threshold (block number).  
//!   If an XP entry is not updated for a sufficient duration,
//!   it becomes inactive and can be reaped.
//!
//!     ```ignore
//!     if timestamp < MinTimeStamp and no active locks:
//!         XP can be reaped
//!     ```
//!
//! - `genesis_acc`: XP identities initialized at genesis.
//!
//! Flow:
//! - Actions build pulse (reputation)
//! - Once pulse reaches [`MinPulse`], XP starts accumulating
//! - Inactivity below [`MinTimeStamp`] allows XP to be reaped
//!
//! - [`Call::force_genesis_config`]  
//!   Restricted to root origin.  
//!   Allows updating these parameters at runtime to adjust system behavior.
//!
//! All genesis parameters are stored in runtime storage and can be updated
//! during runtime; they are not fixed constants.
//!
//! ## Development Feature Gate
//!
//! This pallet includes a `dev` feature gate for development and testing.
//!
//! Core functionality is exposed via public APIs for RPC and UI usage.
//! The `dev` feature provides thin wrapper extrinsics and extended
//! events for direct inspection.
//!
//! This feature must be disabled in production runtimes due to additional debugging overhead.

#![cfg_attr(not(feature = "std"), no_std)]

// ===============================================================================
// `````````````````````````````````` MODULES ````````````````````````````````````
// ===============================================================================

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
mod xp;
mod fungible;
pub mod types;
pub mod weights;

// ===============================================================================
// `````````````````````````````` PALLET MODULE ``````````````````````````````````
// ===============================================================================

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {

    // ===============================================================================
    // ````````````````````````````````` IMPORTS `````````````````````````````````````
    // ===============================================================================

    // --- Core ---
    use core::fmt::Debug;

    // --- Local crate imports ---
    use crate::{
        types::{
            ForceGenesisConfig, GenesisAcc, IdXp, Stepper, Xp, XpEligibility, XpId, XpProgress,
            XpState,
        },
        weights::WeightInfo,
    };

    // --- FRAME Suite ---
    use frame_suite::{
        accumulators::DiscreteAccumulator,
        base::{Asset, Delimited, RuntimeEnum, Time},
        xp::{
            XpLockListener, XpMutate, XpMutateListener, XpOwner, XpOwnerListener, XpReap,
            XpReapListener, XpReserveListener, XpSystem, XpSystemExtensions,
        },
    };

    // --- FRAME Support ---
    use frame_support::{
        dispatch::{DispatchResult, GetDispatchInfo},
        pallet_prelude::*,
        traits::{IsSubType, VariantCount, VariantCountOf},
        Blake2_128Concat,
    };

    // --- FRAME System ---
    use frame_system::{
        ensure_root,
        pallet_prelude::{BlockNumberFor, *},
    };

    // --- External crates ---
    use scale_info::prelude::boxed::Box;

    // --- Substrate crates ---
    use sp_runtime::{traits::Dispatchable, DispatchError, Vec};

    // ===============================================================================
    // `````````````````````````````` PALLET MARKER ``````````````````````````````````
    // ===============================================================================

    /// Primary Marker type for the **XP pallet**.
    ///
    /// This pallet provides implementations for traits from:
    /// - [`xp`](frame_suite::xp)
    /// - [`fungible`](frame_support::traits::fungible)
    ///
    /// ## Fungible Trait Implementations
    ///
    /// The pallet implements the following fungible-related traits:
    ///
    /// - [`Inspect`](frame_support::traits::fungible::Inspect)
    /// - [`Unbalanced`](frame_support::traits::fungible::Unbalanced)
    /// - [`Mutate`](frame_support::traits::fungible::Mutate)
    /// - [`InspectHold`](frame_support::traits::fungible::InspectHold)
    /// - [`InspectFreeze`](frame_support::traits::fungible::InspectFreeze)
    /// - [`UnbalancedHold`](frame_support::traits::fungible::UnbalancedHold)
    /// - [`MutateFreeze`](frame_support::traits::fungible::MutateFreeze)
    /// - [`MutateHold`](frame_support::traits::fungible::MutateHold)
    ///
    /// ## XP Trait Implementations
    ///
    /// [`Pallet`] implements the core XP system traits:
    ///
    /// - [`XpSystem`]
    /// - [`XpOwner`]
    /// - [`XpMutate`]
    /// - [`XpReap`]
    /// - [`XpReserve`](frame_suite::xp::XpReserve)
    /// - [`XpLock`](frame_suite::xp::XpLock)
    ///
    /// ### Helper Traits
    ///
    /// Additional supporting traits:
    ///
    /// - [`DiscreteAccumulator`]
    #[pallet::pallet]
    pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

    // ===============================================================================
    // `````````````````````````````` CONFIG TRAIT ```````````````````````````````````
    // ===============================================================================

    /// Configuration trait for the XP pallet.
    ///
    /// This trait defines the types, constants, and dependencies
    /// that the runtime must provide for this pallet to function.
    ///
    /// The generic parameter `I` allows the same pallet to be instantiated
    /// multiple times within a runtime. Each instance can have its own
    /// independent storage and configuration.
    ///
    /// Example:
    /// - `I = ()` -> default (single instance)
    /// - `I = Core`, `Instance2`, etc. -> multiple independent instances
    #[pallet::config]
    pub trait Config<I: 'static = ()>: frame_system::Config {
        // --- Runtime Anchors ---

        /// The overarching event type.
        type RuntimeEvent: From<Event<Self, I>>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The overarching call type.
        type RuntimeCall: Parameter
            + Dispatchable<RuntimeOrigin = Self::RuntimeOrigin>
            + GetDispatchInfo
            + From<frame_system::Call<Self>>
            + IsSubType<Call<Self, I>>
            + IsType<<Self as frame_system::Config>::RuntimeCall>;

        /// The reason type for XP reserves.
        ///
        /// This should be a bounded, enumerable type (e.g., an enum) that
        /// classifies the context or intent for which XP is reserved (such as
        /// staking, governance, or slashing).
        type ReserveReason: RuntimeEnum + Delimited + Copy + VariantCount;

        /// The reason type for XP locks.
        ///
        /// This should be a bounded, enumerable type (e.g., an enum) that
        /// classifies the context or intent for which XP is locked (such as
        /// staking, governance, or slashing).
        type LockReason: RuntimeEnum + Delimited + Copy + VariantCount;

        // --- Scalars ---

        /// The XP balance type for XP accounting.
        type Xp: Asset + From<Self::Pulse>;

        /// The numeric type used for pulse calculations
        /// (XP activity heartbeat i.e., reputation).
        type Pulse: Time;

        // --- Weights ---

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;

        // --- Extensions ---

        /// XP extensions for external integrations.
        ///
        /// This defines extension hooks that observe XP lifecycle events.
        ///
        /// Note:
        /// - Not intended for consensus-critical logic.
        /// - Use [`frame_suite::Ignore`] for a no-op implementation.
        /// - Invoked regardless of [`Self::EmitEvents`].
        type Extensions: XpSystemExtensions<Via = Pallet<Self, I>>
            + XpOwnerListener
            + XpMutateListener
            + XpReserveListener
            + XpLockListener
            + XpReapListener;

        // --- Constants ---

        /// Controls emission of [`Event`] via `deposit_event`.
        ///
        /// Recommended:
        /// - `false` for production runtimes (to reduce overhead)
        /// - `true` for development and mock runtimes (for testing and
        /// observability)
        #[pallet::constant]
        type EmitEvents: Get<bool> + Clone + Debug;
    }

    // ===============================================================================
    // ``````````````````````````````` GENESIS CONFIG ````````````````````````````````
    // ===============================================================================

    /// Genesis configuration for the XP pallet.
    ///
    /// Defines the initial configuration parameters for the XP pallet,
    /// which are set during the chain's genesis block.
    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config<I>, I: 'static = ()> {
        /// The minimum pulse value required for XP reputation effects.
        ///
        /// This value determines the minimum pulse required for XP entries to be
        /// considered active for reputation calculations or effects.
        pub min_pulse: T::Pulse,

        /// The initial XP assigned to newly created XP entries.
        ///
        /// This value sets the starting XP balance for all XP keys created during
        /// the chain's genesis block or runtime initialization.
        pub init_xp: T::Xp,

        /// The configuration for pulse-based XP activity reputation calculations.
        ///
        /// This field defines the parameters for how pulse is calculated and scaled for reputation effects.
        /// It includes thresholds and scaling factors for determining pulse growth.
        pub pulse_factor: Stepper<T, I>,

        /// XP identities to initialize at genesis.
        ///
        /// Each entry creates an XP identity and assigns its owner.
        /// No XP points are allocated at this stage.
        pub genesis_acc: Vec<GenesisAcc<T::AccountId, XpId<T>>>,
    }

    /// Default values for XP system parameters at genesis.
    impl<T: Config<I>, I: 'static> Default for GenesisConfig<T, I> {
        fn default() -> Self {
            Self {
                min_pulse: 3u32.into(),
                init_xp: 1u32.into(),
                pulse_factor: Stepper::<T, I>::new(50u8.into(), 10u8.into()).unwrap(),
                genesis_acc: Vec::new(),
            }
        }
    }

    /// Builds the XP pallet's genesis storage from the provided configuration.
    #[pallet::genesis_build]
    impl<T: Config<I>, I: 'static> BuildGenesisConfig for GenesisConfig<T, I> {
        fn build(&self) {
            MinPulse::<T, I>::put(self.min_pulse);
            InitXp::<T, I>::put(self.init_xp);
            MinTimeStamp::<T, I>::put(BlockNumberFor::<T>::zero());
            PulseFactor::<T, I>::put(&self.pulse_factor);

            for acc_struct in &self.genesis_acc {
                Pallet::<T, I>::new_xp(&acc_struct.owner, &acc_struct.id)
            }
        }
    }

    // ===============================================================================
    // ``````````````````````````````` STORAGE TYPES `````````````````````````````````
    // ===============================================================================

    /// Stores XP state for key.
    ///
    /// Maps each XP key [`XpId`] to its corresponding XP data structure [`Xp`].
    /// Stores metadata, balances, and activity information for each XP entry.
    #[pallet::storage]
    pub type XpOf<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Blake2_128Concat, XpId<T>, Xp<T, I>, OptionQuery>;

    /// Owner-to-XP-key mapping.
    ///
    /// Maps each account [`frame_system::Config::AccountId`] and XP key [`XpId`]
    /// pair to an empty tuple, representing ownership of the XP key by the account.
    /// Used for efficient owner lookups.
    #[pallet::storage]
    pub type XpOwners<T: Config<I>, I: 'static = ()> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, T::AccountId>,
            NMapKey<Blake2_128Concat, XpId<T>>,
        ),
        (),
        OptionQuery,
    >;

    /// Per-key reserves.
    ///
    /// Maps each XP key [`XpId`] to a bounded vector of reserve entries [`IdXp`],
    /// with the number of reserves limited by the number of enum variants in
    /// [`Config::ReserveReason`].
    ///
    /// Each reserve entry per-key represents XP reserved for a specific reason
    /// or runtime intent.
    #[pallet::storage]
    pub type ReservedXpOf<T: Config<I>, I: 'static = ()> = StorageMap<
        _,
        Blake2_128Concat,
        XpId<T>,
        BoundedVec<IdXp<T::ReserveReason, T::Xp>, VariantCountOf<T::ReserveReason>>,
        OptionQuery,
    >;

    /// Per-key locks (bounded by reason enum).
    ///
    /// Maps each XP key [`XpId`] to a bounded vector of lock entries [`IdXp`],
    /// with the number of locks limited by the number of variants in
    /// [`Config::LockReason`].
    ///
    /// Each lock entry per-key represents XP locked for a specific reason or
    /// runtime intent.
    #[pallet::storage]
    pub type LockedXpOf<T: Config<I>, I: 'static = ()> = StorageMap<
        _,
        Blake2_128Concat,
        XpId<T>,
        BoundedVec<IdXp<T::LockReason, T::Xp>, VariantCountOf<T::LockReason>>,
        OptionQuery,
    >;

    /// Blacklist of finalized (reaped) XP keys.
    ///
    /// Maps each reaped XP key [`XpId`] to an empty tuple, indicating that
    /// the XP entry has been finalized and cannot be recreated or reused.
    #[pallet::storage]
    pub type ReapedXp<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Blake2_128Concat, XpId<T>, (), OptionQuery>;

    /// Minimum pulse required for XP heartbeat/reputation effects.
    ///
    /// Stores the minimum pulse value of type [`Config::Pulse`] that an XP
    /// entry must have to be considered active for reputation or participation
    /// calculations.
    #[pallet::storage]
    pub type MinPulse<T: Config<I>, I: 'static = ()> = StorageValue<_, T::Pulse, ValueQuery>;

    // Initial XP assigned to new XP entries.
    ///
    /// Stores the starting XP value of type [`Config::Xp`] for newly
    /// created XP keys.
    #[pallet::storage]
    pub type InitXp<T: Config<I>, I: 'static = ()> = StorageValue<_, T::Xp, ValueQuery>;

    /// Pulse factor parameters for XP activity reputation.
    ///
    /// Stores the [`Stepper`] struct, which determines how XP pulse (activity heartbeat)
    /// is calculated for reputation effects for all XPs in the system.
    #[pallet::storage]
    pub type PulseFactor<T: Config<I>, I: 'static = ()> =
        StorageValue<_, Stepper<T, I>, ValueQuery>;

    /// Minimum timestamp (block number) for XP liveness.
    ///
    /// Stores the minimum block number of type [`BlockNumberFor`] required
    /// for an XP entry to be considered "alive". Used for XP expiration or
    /// reaping logic.
    #[pallet::storage]
    pub type MinTimeStamp<T: Config<I>, I: 'static = ()> =
        StorageValue<_, BlockNumberFor<T>, ValueQuery>;

    // ===============================================================================
    // ```````````````````````````````````` ERROR ````````````````````````````````````
    // ===============================================================================

    #[pallet::error]
    /// XP Pallet Errors
    pub enum Error<T, I = ()> {
        /// The specified XP key does not exist in the system.
        XpNotFound,
        /// The XP entry is not considered "dead" and cannot be reaped.
        XpNotDead,
        /// The caller is not the owner of the XP key.
        InvalidXpOwner,
        /// The caller is already the owner of the XP key.
        AlreadyXpOwner,
        /// Cannot reap an XP entry that still has active locks.
        CannotReapLockedXp,
        /// A lock with the specified ID/Reason already exists for this XP key.
        XpLockExists,
        /// Failed to deterministically generate an XP key from the provided Preimage.
        CannotGenerateXpKey,
        /// Fungible Transfers are strictly forbidden in the XP system.
        CannotTransferXp,
        /// The provided threshold value is less than the `per_count` value, which is invalid.
        LowPulseThreshold,
        /// Not enough liquid XP to lock the specified amount.
        InsufficientLiquidXp,
        /// Maximum number of locks reached for this XP key.
        TooManyLocks,
        /// Maximum number of reserves reached for this XP key.
        TooManyReserves,
        /// Lock with the specified ID/Reason was not found for this XP key.
        XpLockNotFound,
        /// Reserve with the specified Reason was not found for this XP key.
        XpReserveNotFound,
        /// The minimum timestamp must be less than the current block number.
        InvalidMinTimeStamp,
        /// The XP entry's timestamp is below the minimum required threshold.
        LowTimeStamp,
        /// The XP entry has not been reaped (finalized and removed).
        XpNotReaped,
        /// Pulse-based reputation derivation overflowed.  
        /// Occurs when multiplying XP points by the pulse value overflows the scalar.        
        ReputationDeriveOverflowed,
        /// The maximum capacity of XP was exceeded due to an arithmetic operation.
        XpCapOverflowed,
        /// An arithmetic underflow occurred while subtracting XP points.
        XpCapUnderflowed,
        /// An unexpected error occurred during XP computation.
        /// This is a general error for cases where XP calculations fail due to
        /// unforeseen issues in the logic or data.
        XpComputationError,
        /// Attempted to lock zero XP points (not allowed).
        CannotLockZero,
        /// Attempted to reserve zero XP points (not allowed).
        CannotReserveZero,
        /// The XP entry has already been reaped (finalized) and cannot be reused.
        XpAlreadyReaped,
        /// Not enough reserve XP is available to complete the operation.
        InsufficientReserveXp,
        /// The maximum capacity of XP reserve was exceeded due to an arithmetic operation.
        XpReserveCapOverflowed,
        /// An arithmetic underflow occurred while subtracting reserved XP points.
        XpReserveCapUnderflowed,
        /// The maximum capacity of XP lock was exceeded due to an arithmetic operation.
        XpLockCapOverflowed,
        /// An arithmetic underflow occurred while subtracting locked XP points.
        XpLockCapUnderflowed,
    }

    // ===============================================================================
    // ``````````````````````````````````` EVENTS ````````````````````````````````````
    // ===============================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    /// XP Pallet Events (emitted via `Pallet::deposit_event`)
    pub enum Event<T: Config<I>, I: 'static = ()> {
        /// XP was created or mutated for a given key.
        Xp { id: XpId<T>, xp: T::Xp },
        /// XP ownership was assigned or transferred to a new owner.
        XpOwner { id: XpId<T>, owner: T::AccountId },
        /// XpId's associated with the owner.
        XpOfOwner {
            owner: T::AccountId,
            ids: Vec<XpId<T>>,
        },
        /// XP was earned for the given key.
        XpEarn { id: XpId<T>, xp: T::Xp },
        /// XP entry was reaped (finalized and removed).
        XpReap { id: XpId<T> },
        /// XP points were slashed from an XP entry.
        XpSlash { id: XpId<T>, xp: T::Xp },
        /// XP was locked for a specific runtime intent.
        XpLock {
            of: XpId<T>,
            reason: T::LockReason,
            xp: T::Xp,
        },
        /// A lock was removed (burned) from an XP key.
        XpLockBurn { of: XpId<T>, reason: T::LockReason },
        /// Locked XP points were slashed from an XP key..
        XpLockSlash {
            of: XpId<T>,
            reason: T::LockReason,
            xp: T::Xp,
        },
        /// XP was reserved for a specific runtime intent.
        XpReserve {
            of: XpId<T>,
            reason: T::ReserveReason,
            xp: T::Xp,
        },
        /// Reserved XP points were slashed from an XP key..
        XpReserveSlash {
            of: XpId<T>,
            reason: T::ReserveReason,
            xp: T::Xp,
        },
        /// A genesis config parameter was updated forcefully.
        GenesisConfigUpdated(ForceGenesisConfig<T, I>),
    }

    // ===============================================================================
    // ````````````````````````````````` EXTRINSICS ``````````````````````````````````
    // ===============================================================================

    /// XP Pallet Extrinsics includes major state mutation functions with
    /// origin authentication. Some read only functions are given for
    #[pallet::call]
    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
        // ```````````````````````````````` DISPATCHABLES ````````````````````````````````
        // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

        /// Executes a runtime call using an XP identity as the origin.
        ///
        /// **Origin:** Signed (must be the owner of the XP identity)
        ///
        /// This extrinsic allows the owner of an XP identity to dispatch a call
        /// on its behalf. While an XP identity is not a native account, it can act
        /// as a logical origin for execution through owner authorization.
        ///
        /// The caller must be the registered owner of the given `xp_id`.
        /// Upon successful verification, the provided call is dispatched
        /// with the XP identity as the signed origin.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::call())]
        pub fn call(
            origin: OriginFor<T>,
            xp_id: XpId<T>,
            call: Box<<T as Config<I>>::RuntimeCall>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            Self::is_owner(&caller, &xp_id)?;
            call.dispatch(frame_system::RawOrigin::Signed(xp_id).into())
                .map(|_| ())
                .map_err(|e| e.error)?;
            Ok(())
        }

        /// Transfer or handover ownership of an XP key to another account.
        ///
        /// **Origin:** Signed user (must be the current XP key owner)
        ///
        /// This extrinsic allows the current owner of an XP key to transfer ownership
        /// to another account. The call will fail if the destination account is already
        /// the owner or if the caller does not own the XP key.
        ///
        /// On success, ownership of the XP key is transferred to the target
        /// account and an event is emitted.
        ///
        /// Emits [`Event::XpOwner`] with the XP key and new owner.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::handover())]
        pub fn handover(
            origin: OriginFor<T>,
            xp_id: XpId<T>,
            new_owner: T::AccountId,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            Self::xp_exists(&xp_id)?;
            Self::is_owner(&caller, &xp_id)?;
            ensure!(
                caller != new_owner,
                DispatchError::from(Error::<T, I>::AlreadyXpOwner)
            );
            // Perform the ownership transfer.
            Self::transfer_owner(&caller, &xp_id, &new_owner)?;
            // Emit event purposefully if not yet emitted via earlier call.
            if !T::EmitEvents::get() {
                Self::deposit_event(Event::XpOwner {
                    id: xp_id,
                    owner: new_owner,
                });
            }
            Ok(())
        }

        /// Dispose (Reap) an XP key.
        ///
        /// **Origin:** Signed user
        ///
        /// This extrinsic allows **any** signed account to finalize and remove XP
        /// entries that are no longer valid.
        ///
        /// For an XP key, it checks:
        ///   - The key exists in storage,
        ///   - The key is considered "dead" (does not meet minimum timestamp requirements),
        ///   - The key has no active locks.
        ///
        /// If all checks pass, the XP entry is reaped (removed from storage and blacklisted).
        ///
        /// Emits [`Event::XpReap`] with each successfully reaped XP key.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::dispose())]
        pub fn dispose(
            origin: OriginFor<T>,
            owner: T::AccountId,
            xp_id: XpId<T>,
        ) -> DispatchResult {
            let _caller = ensure_signed(origin)?;
            Self::xp_exists(&xp_id)?;
            Self::is_owner(&owner, &xp_id)?;
            Self::try_reap(&xp_id)?;
            // Emit event purposefully if not yet emitted via earlier call.
            if !T::EmitEvents::get() {
                Self::deposit_event(Event::XpReap { id: xp_id.clone() });
            }
            Ok(())
        }

        // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
        // ````````````````````````````````` INSPECTORS ``````````````````````````````````
        // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

        /// Query the liquid XP balance for an owned XP key.
        ///
        /// **Origin:** Signed user (must be the XP key owner)
        ///
        /// This extrinsic allows the owner of an XP key to query the current liquid XP balance
        /// associated with that key.
        ///
        /// Emits [`Event::Xp`] with the XP key and the current liquid XP balance.
        ///
        /// **Note:** This extrinsic is compiled only when the `dev` feature is enabled.
        /// It is completely excluded from the runtime when `dev` is not enabled,
        /// and therefore is not available in production builds.
        #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::inspect_my_xp())]
        pub fn inspect_my_xp(origin: OriginFor<T>, xp_id: XpId<T>) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            Self::xp_exists(&xp_id)?;
            Self::is_owner(&caller, &xp_id)?;
            // Retrieve the caller's current liquid XP for the key.
            let liquid = Self::xp(&xp_id)?;
            // Deposit Event
            Self::deposit_event(Event::Xp {
                id: xp_id.clone(),
                xp: liquid,
            });
            Ok(())
        }

        /// Emit a snapshot of all XpId's currently owned by the specified account.
        ///
        /// **Origin:** Signed user
        ///
        /// This extrinsic reads the current ownership mapping for `owner`
        /// and emits a single [`Event::XpOfOwner`] containing the complete
        /// list of `XpId`s associated with that account at the time of execution.
        ///
        /// **Note:** This extrinsic is compiled only when the `dev` feature is enabled.
        /// It is completely excluded from the runtime when `dev` is not enabled,
        /// and therefore is not available in production builds.
        #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::inspect_xp_keys_of())]
        pub fn inspect_xp_keys_of(origin: OriginFor<T>, owner: T::AccountId) -> DispatchResult {
            let _caller = ensure_signed(origin)?;
            let xp_ids = Self::xp_keys(&owner)?;
            Self::deposit_event(Event::XpOfOwner {
                owner: owner,
                ids: xp_ids,
            });
            Ok(())
        }

        // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
        // ``````````````````````````````` ROOT PRIVILEGED ```````````````````````````````
        // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

        /// Force transfer/handover ownership of an XP key to another account.
        ///
        /// **Origin:** Root only
        ///
        /// This extrinsic allows the current owner of an XP key to transfer ownership
        /// to another account. The call will fail if the destination account is already
        /// the owner or if the caller does not own the XP key.
        ///
        /// On success, ownership of the XP key is transferred to the target account and
        /// an event is emitted.
        ///
        /// Emits [`Event::XpOwner`] with the XP key and new owner.
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::force_handover())]
        pub fn force_handover(
            origin: OriginFor<T>,
            owner: T::AccountId,
            xp_id: XpId<T>,
            new_owner: T::AccountId,
        ) -> DispatchResult {
            ensure_root(origin)?;
            Self::xp_exists(&xp_id)?;
            Self::is_owner(&owner, &xp_id)?;
            ensure!(
                owner != new_owner,
                DispatchError::from(Error::<T, I>::AlreadyXpOwner)
            );
            // Perform the ownership transfer.
            Self::transfer_owner(&owner, &xp_id, &new_owner)?;
            // Emit event purposefully if not yet emitted via earlier call.
            if !T::EmitEvents::get() {
                Self::deposit_event(Event::XpOwner {
                    id: xp_id.clone(),
                    owner: new_owner.clone(),
                });
            }
            Ok(())
        }

        /// Force-update a selected genesis configuration parameter.
        ///
        /// **Origin:** Root only.
        ///
        /// This extrinsic allows privileged modification of runtime parameters
        /// that were originally defined at genesis.
        ///
        /// The parameter to update is specified via the `ForceGenesisConfig` enum:
        ///
        /// - `MinPulse` - Updates the minimum pulse required for reputation effects.
        /// - `InitXp` - Updates the initial XP assigned to newly created XP entries.
        /// - `PulseFactor` - Updates the pulse stepping configuration
        ///   (`threshold` and `per_count`).
        /// - `MinTimeStamp` - Updated the minimum blocks required
        ///   for an XP entry to be considered alive.
        ///
        /// For `PulseFactor`, the call fails with [`Error::LowPulseThreshold`]
        /// if `per_count > threshold`, as this would invalidate the stepping logic.
        ///
        /// This call directly overwrites storage and emits an event containing the
        /// updated configuration variant.
        #[pallet::call_index(6)]
        #[pallet::weight(
            T::WeightInfo::force_update_init_xp()
                .max(T::WeightInfo::force_update_min_pulse())
                .max(T::WeightInfo::force_update_pulse_factor())
                .max(T::WeightInfo::force_update_min_time_stamp())
        )]
        pub fn force_genesis_config(
            origin: OriginFor<T>,
            field: ForceGenesisConfig<T, I>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            match field {
                ForceGenesisConfig::MinPulse(min_pulse) => MinPulse::<T, I>::set(min_pulse),
                ForceGenesisConfig::InitXp(init_xp) => InitXp::<T, I>::set(init_xp),
                ForceGenesisConfig::PulseFactor {
                    threshold,
                    per_count,
                } => {
                    let Some(stepper) = Stepper::<T, I>::new(threshold, per_count) else {
                        return Err(Error::<T, I>::LowPulseThreshold.into());
                    };
                    PulseFactor::<T, I>::set(stepper);
                }
                ForceGenesisConfig::MinTimeStamp(min_block) => {
                    let current_block = frame_system::Pallet::<T>::block_number();
                    if min_block > current_block {
                        return Err(Error::<T, I>::InvalidMinTimeStamp.into());
                    };
                    MinTimeStamp::<T, I>::set(min_block);
                }
            }
            Self::deposit_event(Event::GenesisConfigUpdated(field));
            Ok(())
        }
    }

    // ===============================================================================
    // `````````````````````````````````` PUBLIC API `````````````````````````````````
    // ===============================================================================

    /// Public read-only functions for inspecting XP balances, reputation,
    /// and pulse progression state.
    ///
    /// This interface exposes non-mutating functions that allow external
    /// consumers (e.g. off-chain clients, RPC layers, other pallets, UI layers,
    /// and gamification engines) to inspect XP ownership, multiplier status,
    /// reputation progress, and simulate `earn_xp` outcomes without modifying
    /// on-chain state.
    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        /// Returns the current XP state snapshot for an identity.
        ///
        /// Combines balances, XP eligibility, and effective multiplier.
        ///
        /// Intended for RPC responses and UI views.
        pub fn xp_state(key: &XpId<T>) -> Result<XpState<T, I>, DispatchError> {
            let xp = Self::get_xp(key)?;

            let eligibility = Self::xp_eligibility(key)?;

            let required_pulse = MinPulse::<T, I>::get();
            let multiplier = match xp.pulse.value < required_pulse {
                true => One::one(),
                false => xp.pulse.value,
            };

            Ok(XpState {
                liquid: xp.free,
                reserved: xp.reserve,
                locked: xp.lock,
                multiplier,
                eligibility,
            })
        }

        /// Returns the current **liquid (free, spendable)** XP of the given `xp_id`.
        ///
        /// This excludes reserved and locked balances.
        pub fn xp(key: &XpId<T>) -> Result<T::Xp, DispatchError> {
            Self::xp_exists(key)?;
            let liquid = Self::get_liquid_xp(key)?;
            Ok(liquid)
        }

        /// Returns all XP IDs owned by the given `owner`.
        pub fn xp_keys(owner: &T::AccountId) -> Result<Vec<XpId<T>>, DispatchError> {
            let xp_ids = Self::xp_of_owner(owner)?;
            Ok(xp_ids)
        }

        /// Checks whether the given XP key can be safely disposed (finalized).
        pub fn is_disposable(key: &XpId<T>) -> DispatchResult {
            Self::can_reap(key)?;
            Ok(())
        }

        /// Returns the XP eligibility state of an identity.
        ///
        /// If XP is already active (`pulse.value >=` [`MinPulse`]), returns `Earning`.
        ///
        /// Otherwise, computes how many additional blocks with at least one
        /// `earn_xp` call are required before XP starts being counted.
        ///
        /// This calculation accounts for:
        /// - Current partial progression toward the next pulse increment
        /// - Pulse threshold
        /// - Progress gained per block (via `earn_xp`)
        ///
        /// Note: Multiple `earn_xp` calls within the same block are treated
        /// as a single progression step.
        ///
        /// Intended for RPC queries, previews, and UI interactions.
        pub fn xp_eligibility(key: &XpId<T>) -> Result<XpEligibility<T, I>, DispatchError> {
            let xp = Self::get_xp(key)?;
            let current_pulse = xp.pulse.value;
            let current_progress = xp.pulse.step;

            let required_pulse = MinPulse::<T, I>::get();
            let pulse_factor = PulseFactor::<T, I>::get();

            // XP already active
            if current_pulse >= required_pulse {
                return Ok(XpEligibility::Earning);
            }

            let threshold = pulse_factor.threshold;
            let per_action = pulse_factor.per_count;

            ensure!(!per_action.is_zero(), Error::<T, I>::XpComputationError);

            let zero = T::Pulse::zero();
            let one = T::Pulse::one();

            let ceil_div_pulse =
                |value: T::Pulse, by: T::Pulse| -> Result<T::Pulse, DispatchError> {
                    ensure!(!by.is_zero(), Error::<T, I>::XpComputationError);

                    let adjusted = value.checked_sub(&one).unwrap_or(zero);

                    adjusted
                        .checked_div(&by)
                        .and_then(|v| v.checked_add(&one))
                        .ok_or(Error::<T, I>::XpComputationError.into())
                };

            // Remaining pulse increments required to activate XP
            let remaining_pulses = required_pulse
                .checked_sub(&current_pulse)
                .ok_or(Error::<T, I>::XpComputationError)?;

            // ceil(threshold / per_action)
            let actions_per_pulse = ceil_div_pulse(threshold, per_action)?;

            // ceil((threshold - current_progress) / per_action)
            let remaining_progress = threshold.checked_sub(&current_progress).unwrap_or(zero);
            let actions_to_next_pulse = ceil_div_pulse(remaining_progress, per_action)?;

            // max(remaining_pulses - 1, 0)
            let extra_pulses = remaining_pulses.checked_sub(&one).unwrap_or(zero);

            let extra_actions = extra_pulses
                .checked_mul(&actions_per_pulse)
                .ok_or(Error::<T, I>::XpComputationError)?;

            let total_actions = actions_to_next_pulse
                .checked_add(&extra_actions)
                .ok_or(Error::<T, I>::XpComputationError)?;

            Ok(XpEligibility::Progressing(total_actions))
        }

        /// Returns the applicable XP multiplier for an identity.
        ///
        /// Once XP is active, the multiplier is derived from the current pulse value.
        /// The multiplier can be applied at most once per block.
        ///
        /// Returns:
        /// - `Some(multiplier)` if a multiplier is available for the next `earn_xp` call
        /// - `None` if no multiplier applies, which occurs when:
        ///   - XP is not valid or active (see [`Self::xp_eligibility`]), or
        ///   - A multiplier has already been applied in the current block
        ///
        /// Note:
        /// - Subsequent `earn_xp` calls within the same block are unscaled.
        ///
        /// Intended for RPC queries, previews, and UI interactions.
        pub fn xp_multiplier(key: &XpId<T>) -> Result<Option<T::Pulse>, DispatchError> {
            let xp = Self::get_xp(key)?;
            let required_pulse = MinPulse::<T, I>::get();

            let multiplier = match xp.pulse.value < required_pulse {
                // XP not yet active -> no multiplier boost
                true => return Ok(None),
                // XP active -> use pulse as multiplier
                false => xp.pulse.value,
            };

            let current_block = frame_system::Pallet::<T>::block_number();

            if xp.timestamp >= current_block {
                return Ok(None);
            }

            Ok(Some(multiplier))
        }

        /// Returns the current XP progression details.
        ///
        /// Includes the current multiplier level, progress toward the next level,
        /// and the configuration that defines how progression advances.
        ///
        /// Intended for UI progress bars and gamified displays.
        pub fn xp_progress(key: &XpId<T>) -> Result<XpProgress<T, I>, DispatchError> {
            let xp = Self::get_xp(key)?;
            let config = PulseFactor::<T, I>::get();

            Ok(XpProgress {
                level: xp.pulse.value,
                progress: xp.pulse.step,
                threshold: config.threshold,
                per_action: config.per_count,
            })
        }

        /// Simulates an `earn_xp` action and returns the resulting XP state.
        ///
        /// Executes the same logic as `earn_xp` without mutating storage,
        /// allowing callers to preview how an action would affect balances,
        /// XP activation, and multiplier.
        ///
        /// Behavior:
        /// - If XP is not yet active, the action contributes only toward activation
        ///   (no reward scaling is applied).
        /// - If XP is active, the input is scaled by the current multiplier (if any).
        /// - Progression toward the next multiplier level is updated accordingly.
        ///
        /// The returned `XpState` reflects the post-action state as if the
        /// operation had been applied.
        ///
        /// Intended for RPC queries, previews, and UI interactions.
        pub fn earn_preview(key: &XpId<T>, raw: T::Xp) -> Result<XpState<T, I>, DispatchError> {
            let xp = Self::get_xp(key)?;

            // compute reward
            let reward = Self::quote_earn_xp(key, raw)?;

            // simulate new balances
            let new_free = xp
                .free
                .checked_add(&reward)
                .ok_or(Error::<T, I>::XpCapOverflowed)?;

            // simulate progression
            let mut next_pulse = xp.pulse.clone();
            let config = PulseFactor::<T, I>::get();

            <Pallet<T, I> as DiscreteAccumulator>::increment(&mut next_pulse, &config);

            // derive next eligibility + multiplier
            let next_key = key; // reuse
            let next_eligibility = match next_pulse.value >= MinPulse::<T, I>::get() {
                true => XpEligibility::Earning,
                false => Self::xp_eligibility(next_key)?,
            };

            let next_multiplier = match next_eligibility {
                XpEligibility::Earning => next_pulse.value,
                _ => T::Pulse::one(),
            };

            Ok(XpState {
                liquid: new_free,
                reserved: xp.reserve,
                locked: xp.lock,
                multiplier: next_multiplier,
                eligibility: next_eligibility,
            })
        }

        /// Returns the block number of the last `earn_xp` execution.
        ///
        /// This value is used to enforce per-block rules, such as:
        /// - Allowing at most one multiplier application per block
        /// - Preventing multiple progression steps within the same block
        ///
        /// Intended for RPC queries, previews, and UI interactions.
        pub fn xp_last_earn(key: &XpId<T>) -> Result<BlockNumberFor<T>, DispatchError> {
            let xp = Self::get_xp(key)?;
            Ok(xp.timestamp)
        }
    }
}

// ===============================================================================
// `````````````````````````````````` API TESTS ``````````````````````````````````
// ===============================================================================

/// Unit tests for Extrinsics and Public APIs of [`pallet_xp`](crate).
#[cfg(test)]
mod ext_tests {

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` IMPORTS ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    // --- Local crate imports ---
    use crate::{
        mock::*,
        types::{ForceGenesisConfig, IdXp, XpEligibility},
    };

    // --- FRAME Suite ---
    use frame_suite::xp::{XpLock, XpMutate, XpOwner, XpReserve, XpSystem};

    // --- FRAME Support ---
    use frame_support::{assert_err, assert_ok, traits::VariantCountOf};

    // --- Substrate primitives ---
    use sp_runtime::{BoundedVec, DispatchError};

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // `````````````````````````````` STORAGE INSTANCES ``````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn pulse_factor_instance_check() {
        xp_test_ext().execute_with(|| {
            let threshold_1 = 100;
            let per_count_1 = 10;

            let threshold_2 = 1000;
            let per_count_2 = 100;

            let old_pulsefactor_instance1 = PulseFactor::get();
            let old_pulsefactor_instance2 = PulseFactor2::get();
            assert_eq!(
                old_pulsefactor_instance1,
                Stepper::new(50u8.into(), 10u8.into()).unwrap(),
            );
            assert_eq!(
                old_pulsefactor_instance2,
                Stepper2::new(20u8.into(), 6u8.into()).unwrap(),
            );

            let stepper_1 = Stepper::new(threshold_1, per_count_1).unwrap();
            let stepper_2 = Stepper2::new(threshold_2, per_count_2).unwrap();

            PulseFactor::set(stepper_1.clone());
            PulseFactor2::set(stepper_2.clone());

            assert_eq!(PulseFactor::get(), stepper_1);
            assert_eq!(PulseFactor2::get(), stepper_2);
        });
    }

    #[test]
    fn min_pulse_instance_check() {
        xp_test_ext().execute_with(|| {
            let min_pulse_1 = 10;
            let min_pulse_2 = 15;

            let old_minpulse_instance1 = MinPulse::get();
            let old_min_pulse_instance2 = MinPulse2::get();
            assert_eq!(old_minpulse_instance1, 1);
            assert_eq!(old_min_pulse_instance2, 5);

            MinPulse::set(min_pulse_1);
            MinPulse2::set(min_pulse_2);
            assert_eq!(MinPulse::get(), 10);
            assert_eq!(MinPulse2::get(), 15);
        });
    }

    #[test]
    fn init_xp_instance_check() {
        xp_test_ext().execute_with(|| {
            let init_xp_1 = 5;
            let init_xp_2 = 3;

            let old_initxp_instance1 = InitXp::get();
            let old_initxp_instance2 = InitXp2::get();
            assert_eq!(old_initxp_instance1, 10);
            assert_eq!(old_initxp_instance2, 1);

            InitXp::set(init_xp_1);
            InitXp2::set(init_xp_2);
            assert_eq!(InitXp::get(), 5);
            assert_eq!(InitXp2::get(), 3);
        });
    }

    #[test]
    fn min_time_stamp_instance_check() {
        xp_test_ext().execute_with(|| {
            let min_time_stamp_1 = 5;
            let min_time_stamp_2 = 10;

            let old_mintimestamp_instance1 = MinTimeStamp::get();
            let old_mintimestamp_instance2 = MinTimeStamp2::get();
            assert_eq!(old_mintimestamp_instance1, 0);
            assert_eq!(old_mintimestamp_instance2, 0);

            MinTimeStamp::set(min_time_stamp_1);
            MinTimeStamp2::set(min_time_stamp_2);
            assert_eq!(MinTimeStamp::get(), 5);
            assert_eq!(MinTimeStamp2::get(), 10);
        });
    }

    #[test]
    fn xp_of_instance_check() {
        xp_test_ext().execute_with(|| {
            let xp_1 = MockXp::default();
            XpOf::insert(XP_ALPHA, xp_1);

            let xp_2 = MockXp2::default();
            XpOf2::insert(XP_BETA, xp_2);

            assert!(XpOf::contains_key(XP_ALPHA));
            assert!(XpOf2::contains_key(XP_BETA));

            assert!(!XpOf::contains_key(XP_BETA));
            assert!(!XpOf2::contains_key(XP_ALPHA));
        });
    }

    #[test]
    fn xp_owners_instance_check() {
        xp_test_ext().execute_with(|| {
            XpOwners::insert((ALICE, XP_ALPHA), ());

            XpOwners2::insert((BOB, XP_BETA), ());

            assert!(XpOwners::contains_key((ALICE, XP_ALPHA)));
            assert!(XpOwners2::contains_key((BOB, XP_BETA)));
            assert!(!XpOwners::contains_key((BOB, XP_BETA)));
            assert!(!XpOwners2::contains_key((ALICE, XP_ALPHA)));
        });
    }

    #[test]
    fn reserved_xp_of_instance_check() {
        xp_test_ext().execute_with(|| {
            let reserve_1 = IdXp::new(STAKING, DEFAULT_POINTS);

            ReservedXpOf::try_mutate(XP_ALPHA, |value| {
                let vec = value.get_or_insert_with(|| {
                    BoundedVec::<IdXp<Reason, u64>, VariantCountOf<Reason>>::default()
                });
                vec.try_push(reserve_1)
            })
            .unwrap();

            let reserve_2 = IdXp::new(GOVERNANCE, DEFAULT_POINTS);

            ReservedXpOf2::try_mutate(XP_BETA, |value| {
                let vec = value.get_or_insert_with(|| {
                    BoundedVec::<IdXp<Reason, u64>, VariantCountOf<Reason>>::default()
                });
                vec.try_push(reserve_2)
            })
            .unwrap();

            assert!(ReservedXpOf::contains_key(XP_ALPHA));
            assert!(ReservedXpOf2::contains_key(XP_BETA));
            assert!(!ReservedXpOf::contains_key(XP_BETA));
            assert!(!ReservedXpOf2::contains_key(XP_ALPHA));
        });
    }

    #[test]
    fn locked_xp_of_instance_check() {
        xp_test_ext().execute_with(|| {
            let lock_1 = IdXp::new(STAKING, DEFAULT_POINTS);

            LockedXpOf::try_mutate(XP_ALPHA, |value| {
                let vec = value.get_or_insert_with(|| {
                    BoundedVec::<IdXp<Reason, u64>, VariantCountOf<Reason>>::default()
                });
                vec.try_push(lock_1)
            })
            .unwrap();

            let lock_2 = IdXp::new(GOVERNANCE, DEFAULT_POINTS);
            LockedXpOf2::try_mutate(XP_BETA, |value| {
                let vec = value.get_or_insert_with(|| {
                    BoundedVec::<IdXp<Reason, u64>, VariantCountOf<Reason>>::default()
                });
                vec.try_push(lock_2)
            })
            .unwrap();

            assert!(LockedXpOf::contains_key(XP_ALPHA));
            assert!(LockedXpOf2::contains_key(XP_BETA));
            assert!(!LockedXpOf::contains_key(XP_BETA));
            assert!(!LockedXpOf2::contains_key(XP_ALPHA));
        });
    }

    #[test]
    fn reaped_xp_instance_check() {
        xp_test_ext().execute_with(|| {
            ReapedXp::insert(XP_ALPHA, ());
            ReapedXp2::insert(XP_BETA, ());

            assert!(ReapedXp::contains_key(XP_ALPHA));
            assert!(ReapedXp2::contains_key(XP_BETA));

            assert!(!ReapedXp::contains_key(XP_BETA));
            assert!(!ReapedXp2::contains_key(XP_ALPHA));
        });
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // `````````````````````````````````` PUBLIC API `````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn xp_eligibility_success_already_reputed() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let min_pulse = MinPulse::get();
            assert!(xp.pulse.value < min_pulse);

            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();

            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let min_pulse = MinPulse::get();
            assert!(xp.pulse.value >= min_pulse);

            let status = Pallet::xp_eligibility(&XP_ALPHA).unwrap();
            assert_eq!(status, XpEligibility::Earning);
        })
    }

    #[test]
    fn xp_eligibility_success_edge_cases() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            // Instance1:
            // threshold = 50
            // per_count = 10
            let stepper = Stepper::new(20, 6).unwrap();
            PulseFactor::put(stepper);

            // calls_per_full_pulse = ceil(20 / 6) = 4
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();

            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let min_pulse = MinPulse::get();
            assert!(xp.pulse.value < min_pulse);
            assert_eq!(xp.pulse.step, 12);

            let status = Pallet::xp_eligibility(&XP_ALPHA).unwrap();
            assert_eq!(status, XpEligibility::Progressing(2));

            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();

            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let min_pulse = MinPulse::get();
            assert!(xp.pulse.value >= min_pulse);
            assert_eq!(xp.pulse.step, 4);

            let status = Pallet::xp_eligibility(&XP_ALPHA).unwrap();
            assert_eq!(status, XpEligibility::Earning);
        })
    }

    #[test]
    fn xp_eligibility_success_calls_to_reach_reputed() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let min_pulse = MinPulse::get();
            assert!(xp.pulse.value < min_pulse);

            let status = Pallet::xp_eligibility(&XP_ALPHA).unwrap();
            assert_eq!(status, XpEligibility::Progressing(5));

            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();

            let status = Pallet::xp_eligibility(&XP_ALPHA).unwrap();

            assert_eq!(status, XpEligibility::Progressing(4));

            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();

            let status = Pallet::xp_eligibility(&XP_ALPHA).unwrap();

            assert_eq!(status, XpEligibility::Progressing(2));

            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();

            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let min_pulse = MinPulse::get();
            assert!(xp.pulse.value >= min_pulse);

            let status = Pallet::xp_eligibility(&XP_ALPHA).unwrap();
            assert_eq!(status, XpEligibility::Earning);
        })
    }

    #[test]
    fn xp_multiplier_less_than_min_pulse() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);

            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();

            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let min_pulse = MinPulse::get();
            assert!(xp.pulse.value < min_pulse);

            let current_multiplier = Pallet::xp_multiplier(&XP_ALPHA).unwrap();
            assert!(current_multiplier.is_none());
        })
    }

    #[test]
    fn xp_multiplier_same_block_protection() {
        xp_test_ext().execute_with(|| {
            System::set_block_number(1);
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            System::set_block_number(12);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(13);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(14);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(15);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(16);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(17);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();

            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let min_pulse = MinPulse::get();
            assert!(xp.pulse.value == min_pulse);
            let current_multiplier = Pallet::xp_multiplier(&XP_ALPHA).unwrap();
            assert!(current_multiplier.is_none());
        })
    }

    #[test]
    fn xp_multiplier_success() {
        xp_test_ext().execute_with(|| {
            System::set_block_number(1);
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            System::set_block_number(12);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(13);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(14);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(15);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(16);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();

            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let min_pulse = MinPulse::get();
            assert!(xp.pulse.value == min_pulse);
            System::set_block_number(17);
            let current_multiplier = Pallet::xp_multiplier(&XP_ALPHA).unwrap();
            assert_eq!(current_multiplier, Some(1));

            Pallet::set_lock(&XP_ALPHA, &STAKING, DEFAULT_POINTS).unwrap();
            System::set_block_number(20);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(21);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(22);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(23);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(24);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();

            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let min_pulse = MinPulse::get();
            assert!(xp.pulse.value > min_pulse);
            dbg!(xp.pulse.value);
            System::set_block_number(25);
            let current_multiplier = Pallet::xp_multiplier(&XP_ALPHA).unwrap();
            assert_eq!(current_multiplier, Some(2));
        })
    }

    #[test]
    fn xp_state_success() {
        xp_test_ext().execute_with(|| {
            System::set_block_number(5);
            Pallet::new_xp(&ALICE, &XP_ALPHA);

            System::set_block_number(20);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(21);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(22);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();

            let xp_state = Pallet::xp_state(&XP_ALPHA).unwrap();
            assert_eq!(xp_state.liquid, 10);
            assert_eq!(xp_state.reserved, 0);
            assert_eq!(xp_state.locked, 0);
            assert_eq!(xp_state.multiplier, 1);
            assert_eq!(xp_state.eligibility, XpEligibility::Progressing(1));

            System::set_block_number(23);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            Pallet::set_lock(&XP_ALPHA, &STAKING, DEFAULT_POINTS).unwrap();
            Pallet::set_reserve(&XP_ALPHA, &STAKING, 25).unwrap();

            System::set_block_number(24);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();

            let xp_state = Pallet::xp_state(&XP_ALPHA).unwrap();
            assert_eq!(xp_state.liquid, 20);
            assert_eq!(xp_state.reserved, 25);
            assert_eq!(xp_state.locked, DEFAULT_POINTS);
            assert_eq!(xp_state.multiplier, 1);
            assert_eq!(xp_state.eligibility, XpEligibility::Earning);
        })
    }

    #[test]
    fn fetch_pulse_progress() {
        xp_test_ext().execute_with(|| {
            System::set_block_number(5);
            Pallet::new_xp(&ALICE, &XP_ALPHA);

            System::set_block_number(20);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(21);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();

            let pulse_progress = Pallet::xp_progress(&XP_ALPHA).unwrap();
            assert_eq!(pulse_progress.progress, 20);
            assert_eq!(pulse_progress.level, 0);
            assert_eq!(pulse_progress.threshold, 50);
            assert_eq!(pulse_progress.per_action, 10);

            System::set_block_number(22);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(23);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(24);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();

            let pulse_progress = Pallet::xp_progress(&XP_ALPHA).unwrap();
            assert_eq!(pulse_progress.progress, 0);
            assert_eq!(pulse_progress.level, 1);
            assert_eq!(pulse_progress.threshold, 50);
            assert_eq!(pulse_progress.per_action, 10);
        })
    }

    #[test]
    fn earn_preview_below_min_pulse_returns_zero_reward_and_required_steps() {
        xp_test_ext().execute_with(|| {
            System::set_block_number(10);
            Pallet::new_xp(&ALICE, &XP_ALPHA);

            let earn_preview = Pallet::earn_preview(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            assert_eq!(earn_preview.liquid, DEFAULT_POINTS);
            assert_eq!(earn_preview.reserved, 0);
            assert_eq!(earn_preview.locked, 0);
            assert_eq!(earn_preview.multiplier, 1);
            assert_eq!(earn_preview.eligibility, XpEligibility::Progressing(5));
        })
    }

    #[test]
    fn earn_preview_will_repute_progress() {
        xp_test_ext().execute_with(|| {
            System::set_block_number(10);
            Pallet::new_xp(&ALICE, &XP_ALPHA);

            System::set_block_number(22);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(23);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(24);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();

            System::set_block_number(25);
            let earn_preview = Pallet::earn_preview(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            assert_eq!(earn_preview.liquid, DEFAULT_POINTS);
            assert_eq!(earn_preview.reserved, 0);
            assert_eq!(earn_preview.locked, 0);
            assert_eq!(earn_preview.multiplier, 1);
            assert_eq!(earn_preview.eligibility, XpEligibility::Progressing(2));

            System::set_block_number(25);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();

            System::set_block_number(26);
            let earn_preview = Pallet::earn_preview(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            assert_eq!(earn_preview.liquid, DEFAULT_POINTS);
            assert_eq!(earn_preview.reserved, 0);
            assert_eq!(earn_preview.locked, 0);
            assert_eq!(earn_preview.multiplier, 1);
            assert_eq!(earn_preview.eligibility, XpEligibility::Earning);
        })
    }

    #[test]
    fn earn_preview_above_min_pulse() {
        xp_test_ext().execute_with(|| {
            System::set_block_number(10);
            Pallet::new_xp(&ALICE, &XP_ALPHA);

            System::set_block_number(22);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(23);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(24);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(25);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(26);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();

            System::set_block_number(27);
            let earn_preview = Pallet::earn_preview(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            assert_eq!(earn_preview.liquid, 20);
            assert_eq!(earn_preview.reserved, 0);
            assert_eq!(earn_preview.locked, 0);
            assert_eq!(earn_preview.multiplier, 1);
            assert_eq!(earn_preview.eligibility, XpEligibility::Earning);
        })
    }

    #[test]
    fn earn_preview_multiplier_progress_without_lock() {
        xp_test_ext().execute_with(|| {
            System::set_block_number(10);
            Pallet::new_xp(&ALICE, &XP_ALPHA);

            // Build reputation to reach MinPulse
            System::set_block_number(22);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(23);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(24);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(25);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(26);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();

            System::set_block_number(27);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            // same-block
            let earn_preview = Pallet::earn_preview(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            assert_eq!(earn_preview.liquid, 30);
            assert_eq!(earn_preview.reserved, 0);
            assert_eq!(earn_preview.locked, 0);
            assert_eq!(earn_preview.multiplier, 1);
            assert_eq!(earn_preview.eligibility, XpEligibility::Earning);

            for n in 28..48 {
                System::set_block_number(n);
                Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            }
            // multiplier not increased without lock
            let earn_preview = Pallet::earn_preview(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            assert_eq!(earn_preview.liquid, 230);
            assert_eq!(earn_preview.reserved, 0);
            assert_eq!(earn_preview.locked, 0);
            assert_eq!(earn_preview.multiplier, 1);
            assert_eq!(earn_preview.eligibility, XpEligibility::Earning);
        })
    }

    #[test]
    fn earn_preview_with_lock() {
        xp_test_ext().execute_with(|| {
            System::set_block_number(10);
            Pallet::new_xp(&ALICE, &XP_ALPHA);

            System::set_block_number(22);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(23);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(24);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(25);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(26);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();

            Pallet::set_lock(&XP_ALPHA, &STAKING, DEFAULT_POINTS).unwrap();

            System::set_block_number(27);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(28);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();

            System::set_block_number(29);
            let earn_preview = Pallet::earn_preview(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            assert_eq!(earn_preview.liquid, 40);
            assert_eq!(earn_preview.reserved, 0);
            assert_eq!(earn_preview.locked, 10);
            assert_eq!(earn_preview.multiplier, 1);
            assert_eq!(earn_preview.eligibility, XpEligibility::Earning);

            System::set_block_number(30);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(31);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();

            System::set_block_number(32);
            let earn_preview = Pallet::earn_preview(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            assert_eq!(earn_preview.liquid, 60);
            assert_eq!(earn_preview.reserved, 0);
            assert_eq!(earn_preview.locked, 10);
            assert_eq!(earn_preview.multiplier, 2);
            assert_eq!(earn_preview.eligibility, XpEligibility::Earning);
        })
    }

    #[test]
    fn earn_preview_with_lock_multiplier_progress() {
        xp_test_ext().execute_with(|| {
            System::set_block_number(10);
            Pallet::new_xp(&ALICE, &XP_ALPHA);

            System::set_block_number(22);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(23);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(24);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(25);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(26);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();

            Pallet::set_lock(&XP_ALPHA, &STAKING, DEFAULT_POINTS).unwrap();

            System::set_block_number(27);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(28);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(29);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(30);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();

            System::set_block_number(31);
            let earn_preview = Pallet::earn_preview(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            assert_eq!(earn_preview.liquid, 60);
            assert_eq!(earn_preview.reserved, 0);
            assert_eq!(earn_preview.locked, 10);
            assert_eq!(earn_preview.multiplier, 2);
            assert_eq!(earn_preview.eligibility, XpEligibility::Earning);

            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();

            for n in 32..42 {
                System::set_block_number(n);
                Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            }

            // multiplier increased with lock
            let earn_preview = Pallet::earn_preview(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            assert_eq!(earn_preview.liquid, 320);
            assert_eq!(earn_preview.reserved, 0);
            assert_eq!(earn_preview.locked, 10);
            assert_eq!(earn_preview.multiplier, 4);
            assert_eq!(earn_preview.eligibility, XpEligibility::Earning);
        })
    }

    #[test]
    fn earn_preview_with_same_block_protection() {
        xp_test_ext().execute_with(|| {
            System::set_block_number(10);
            Pallet::new_xp(&ALICE, &XP_ALPHA);

            System::set_block_number(22);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(23);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(24);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(25);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(26);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();

            Pallet::set_lock(&XP_ALPHA, &STAKING, DEFAULT_POINTS).unwrap();

            System::set_block_number(27);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(28);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(29);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(30);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            System::set_block_number(31);
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();

            let earn_preview = Pallet::earn_preview(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            assert_eq!(earn_preview.liquid, 70);
            assert_eq!(earn_preview.reserved, 0);
            assert_eq!(earn_preview.locked, 10);
            assert_eq!(earn_preview.multiplier, 2);
            assert_eq!(earn_preview.eligibility, XpEligibility::Earning);
        })
    }

    #[test]
    fn earn_preview_matches_earn_xp_actual_reward() {
        xp_test_ext().execute_with(|| {
            System::set_block_number(10);
            Pallet::new_xp(&ALICE, &XP_ALPHA);

            Pallet::set_lock(&XP_ALPHA, &STAKING, DEFAULT_POINTS).unwrap();

            // Build to reputed state and increase multiplier with lock
            for n in 20..40 {
                System::set_block_number(n);
                Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            }

            System::set_block_number(41);

            let earn_preview = Pallet::earn_preview(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            assert_eq!(earn_preview.liquid, 350);
            assert_eq!(earn_preview.reserved, 0);
            assert_eq!(earn_preview.locked, 10);
            assert_eq!(earn_preview.multiplier, 4);
            assert_eq!(earn_preview.eligibility, XpEligibility::Earning);

            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let free_before = xp.free;

            let actual_earn = Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let free_after = xp.free;

            let diff = free_after - free_before;
            assert_eq!(free_after, earn_preview.liquid);
            assert_eq!(diff, actual_earn);
        })
    }

    #[test]
    fn earn_preview_err_xp_not_found() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            assert_err!(
                Pallet::earn_preview(&XP_BETA, DEFAULT_POINTS),
                Error::XpNotFound
            );
        })
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // `````````````````````````````````` EXTRINSICS `````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[cfg(feature = "dev")]
    #[test]
    fn inspect_my_xp_success() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            System::set_block_number(1);
            assert_ok!(Xp::inspect_my_xp(RuntimeOrigin::signed(ALICE), XP_ALPHA));
            System::assert_last_event(
                Event::Xp {
                    id: XP_ALPHA,
                    xp: InitXp::get(),
                }
                .into(),
            );
        });
    }

    #[cfg(feature = "dev")]
    #[test]
    fn inspect_my_xp_fail_xp_not_found() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            assert_err!(
                Xp::inspect_my_xp(RuntimeOrigin::signed(ALICE), XP_BETA),
                Error::XpNotFound
            );
        });
    }

    #[cfg(feature = "dev")]
    #[test]
    fn inspect_my_xp_fail_not_signed() {
        xp_test_ext().execute_with(|| {
            assert_err!(
                Xp::inspect_my_xp(RuntimeOrigin::root(), XP_ALPHA),
                DispatchError::BadOrigin
            );
        });
    }

    #[cfg(feature = "dev")]
    #[test]
    fn inspect_my_xp_fail_invalid_owner() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            assert_err!(
                Xp::inspect_my_xp(RuntimeOrigin::signed(BOB), XP_ALPHA),
                Error::InvalidXpOwner
            );
        });
    }

    #[test]
    fn handover_success() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            System::set_block_number(1);
            assert_ok!(Xp::handover(RuntimeOrigin::signed(ALICE), XP_ALPHA, BOB));
            assert_ok!(Pallet::is_owner(&BOB, &XP_ALPHA));
            System::assert_last_event(
                Event::XpOwner {
                    id: XP_ALPHA,
                    owner: BOB,
                }
                .into(),
            );
        });
    }

    #[test]
    fn handover_fail_xp_not_found() {
        xp_test_ext().execute_with(|| {
            assert_err!(
                Xp::handover(RuntimeOrigin::signed(ALICE), XP_ALPHA, BOB),
                Error::XpNotFound
            );
        });
    }

    #[test]
    fn handover_fail_not_signed() {
        xp_test_ext().execute_with(|| {
            assert_err!(
                Xp::handover(RuntimeOrigin::root(), XP_ALPHA, BOB),
                DispatchError::BadOrigin
            );
        });
    }

    #[test]
    fn handover_fail_invalid_owner() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            assert_err!(
                Xp::handover(RuntimeOrigin::signed(CHARLIE), XP_ALPHA, BOB),
                Error::InvalidXpOwner
            );
        });
    }

    #[test]
    fn handover_fail_already_owner() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            assert_err!(
                Xp::handover(RuntimeOrigin::signed(ALICE), XP_ALPHA, ALICE),
                Error::AlreadyXpOwner
            );
        });
    }

    #[test]
    fn dispose_success() {
        xp_test_ext().execute_with(|| {
            MinTimeStamp::set(3);
            System::set_block_number(1);
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            Pallet::set_xp(&XP_ALPHA, 0).unwrap();
            assert_ok!(Pallet::xp_exists(&XP_ALPHA));
            System::set_block_number(2);
            assert_ok!(Xp::dispose(RuntimeOrigin::signed(CHARLIE), ALICE, XP_ALPHA));
            assert_err!(Pallet::xp_exists(&XP_ALPHA), Error::XpNotFound);
        });
    }

    #[test]
    fn dispose_fail_xp_not_found() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);

            assert_err!(
                Xp::dispose(RuntimeOrigin::signed(CHARLIE), ALICE, XP_BETA),
                Error::XpNotFound
            );
        });
    }

    #[test]
    fn dispose_fail_not_owner() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            assert_err!(
                Xp::dispose(RuntimeOrigin::signed(CHARLIE), BOB, XP_ALPHA),
                Error::InvalidXpOwner
            );
        });
    }

    #[test]
    fn dispose_fail_xp_not_dead() {
        xp_test_ext().execute_with(|| {
            System::set_block_number(1);
            System::set_block_number(2);
            System::set_block_number(3);
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            Pallet::set_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            assert_err!(
                Xp::dispose(RuntimeOrigin::signed(CHARLIE), ALICE, XP_ALPHA),
                Error::XpNotDead
            );
        });
    }

    #[test]
    fn dispose_fail_locked_xp() {
        xp_test_ext().execute_with(|| {
            MinTimeStamp::set(3);
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            Pallet::set_lock(&XP_ALPHA, &STAKING, DEFAULT_POINTS).unwrap();
            System::set_block_number(2);
            assert_err!(
                Xp::dispose(RuntimeOrigin::signed(CHARLIE), ALICE, XP_ALPHA),
                Error::CannotReapLockedXp
            );
        });
    }

    #[test]
    fn force_handover_success() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            System::set_block_number(1);
            assert_ok!(Xp::force_handover(
                RuntimeOrigin::root(),
                ALICE,
                XP_ALPHA,
                BOB
            ));
            assert_ok!(Pallet::is_owner(&BOB, &XP_ALPHA));
            System::assert_last_event(
                Event::XpOwner {
                    id: XP_ALPHA,
                    owner: BOB,
                }
                .into(),
            );
        });
    }

    #[test]
    fn force_handover_fail_xp_not_found() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            assert_err!(
                Xp::force_handover(RuntimeOrigin::root(), ALICE, XP_BETA, BOB),
                Error::XpNotFound
            );
        });
    }

    #[test]
    fn force_handover_fail_not_root() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            assert_err!(
                Xp::force_handover(RuntimeOrigin::signed(CHARLIE), ALICE, XP_ALPHA, BOB),
                DispatchError::BadOrigin
            );
        });
    }

    #[test]
    fn force_handover_fail_invalid_owner() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            assert_err!(
                Xp::force_handover(RuntimeOrigin::root(), CHARLIE, XP_ALPHA, BOB),
                Error::InvalidXpOwner
            );
        });
    }

    #[test]
    fn force_handover_fail_already_owner() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            assert_err!(
                Xp::force_handover(RuntimeOrigin::root(), ALICE, XP_ALPHA, ALICE),
                Error::AlreadyXpOwner
            );
        });
    }

    #[cfg(feature = "dev")]
    #[test]
    fn inspect_xp_keys_of_success() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            Pallet::new_xp(&ALICE, &XP_BETA);
            System::set_block_number(1);
            assert_ok!(Xp::inspect_xp_keys_of(RuntimeOrigin::signed(ALICE), ALICE));
            System::assert_last_event(
                Event::XpOfOwner {
                    owner: ALICE,
                    ids: vec![XP_ALPHA, XP_BETA],
                }
                .into(),
            );
        });
    }

    #[cfg(feature = "dev")]
    #[test]
    fn inspect_xp_keys_of_fail_not_signed() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            Pallet::new_xp(&ALICE, &XP_BETA);
            assert_err!(
                Xp::inspect_xp_keys_of(RuntimeOrigin::root(), ALICE),
                DispatchError::BadOrigin
            );
        });
    }

    #[test]
    fn force_genesis_config_min_pulse_success() {
        xp_test_ext().execute_with(|| {
            System::set_block_number(1);
            let new_min_pulse: u32 = 5;
            assert_ok!(Xp::force_genesis_config(
                RuntimeOrigin::root(),
                ForceGenesisConfig::MinPulse(new_min_pulse)
            ));
            assert_eq!(MinPulse::get(), new_min_pulse);

            System::assert_last_event(
                Event::GenesisConfigUpdated(ForceGenesisConfig::MinPulse(new_min_pulse)).into(),
            );
        });
    }

    #[test]
    fn force_genesis_config_min_pulse_fail_not_root() {
        xp_test_ext().execute_with(|| {
            let min_pulse = 5;
            assert_err!(
                Xp::force_genesis_config(
                    RuntimeOrigin::signed(CHARLIE),
                    ForceGenesisConfig::MinPulse(min_pulse)
                ),
                DispatchError::BadOrigin
            );
            assert_eq!(MinPulse::get(), 1);
        });
    }

    #[test]
    fn force_genesis_config_init_xp_success() {
        xp_test_ext().execute_with(|| {
            System::set_block_number(1);
            let new_init_xp = 50;
            assert_ok!(Xp::force_genesis_config(
                RuntimeOrigin::root(),
                ForceGenesisConfig::InitXp(new_init_xp)
            ));
            assert_eq!(InitXp::get(), new_init_xp);
            System::assert_last_event(
                Event::GenesisConfigUpdated(ForceGenesisConfig::InitXp(new_init_xp)).into(),
            );
        });
    }

    #[test]
    fn force_genesis_config_init_xp_fail_not_root() {
        xp_test_ext().execute_with(|| {
            let new_init_xp = 50;
            assert_err!(
                Xp::force_genesis_config(
                    RuntimeOrigin::signed(CHARLIE),
                    ForceGenesisConfig::InitXp(new_init_xp)
                ),
                DispatchError::BadOrigin
            );
            assert_eq!(InitXp::get(), 10);
        });
    }

    #[test]
    fn force_genesis_config_min_time_stamp_success() {
        xp_test_ext().execute_with(|| {
            System::set_block_number(1);
            let new_min_time_stamp = 4;
            System::set_block_number(5);
            assert_ok!(Xp::force_genesis_config(
                RuntimeOrigin::root(),
                ForceGenesisConfig::MinTimeStamp(new_min_time_stamp)
            ));
            assert_eq!(MinTimeStamp::get(), new_min_time_stamp);
            System::assert_last_event(
                Event::GenesisConfigUpdated(ForceGenesisConfig::MinTimeStamp(new_min_time_stamp))
                    .into(),
            );
        });
    }

    #[test]
    fn force_genesis_config_min_time_stamp_fail_not_root() {
        xp_test_ext().execute_with(|| {
            let new_min_time_stamp = 4;
            assert_err!(
                Xp::force_genesis_config(
                    RuntimeOrigin::signed(ALICE),
                    ForceGenesisConfig::MinTimeStamp(new_min_time_stamp)
                ),
                DispatchError::BadOrigin
            );
        });
    }

    #[test]
    fn force_genesis_config_min_time_stamp_fail_invalid_min_time_stamp() {
        xp_test_ext().execute_with(|| {
            let new_min_time_stamp = 4;
            // min_time_stamp > current block number
            System::set_block_number(3);
            assert_err!(
                Xp::force_genesis_config(
                    RuntimeOrigin::root(),
                    ForceGenesisConfig::MinTimeStamp(new_min_time_stamp)
                ),
                Error::InvalidMinTimeStamp
            );
        });
    }

    #[test]
    fn force_genesis_config_pulse_factor_success() {
        xp_test_ext().execute_with(|| {
            System::set_block_number(1);
            let threshold = 100;
            let per_count = 10;
            assert_ok!(Xp::force_genesis_config(
                RuntimeOrigin::root(),
                ForceGenesisConfig::PulseFactor {
                    threshold,
                    per_count
                }
            ));
            let stepper = PulseFactor::get();
            assert_eq!(stepper.threshold, threshold);
            assert_eq!(stepper.per_count, per_count);
            System::assert_last_event(
                Event::GenesisConfigUpdated(ForceGenesisConfig::PulseFactor {
                    threshold,
                    per_count,
                })
                .into(),
            );
        })
    }

    #[test]
    fn force_genesis_config_pulse_factor_fail_low_pulse_threshold() {
        xp_test_ext().execute_with(|| {
            let threshold = 100;
            let per_count = 110;
            assert_err!(
                Xp::force_genesis_config(
                    RuntimeOrigin::root(),
                    ForceGenesisConfig::PulseFactor {
                        threshold,
                        per_count
                    }
                ),
                Error::LowPulseThreshold
            );
        });
    }

    #[test]
    fn force_genesis_config_pulse_factor_fail_not_root() {
        xp_test_ext().execute_with(|| {
            let threshold = 100;
            let per_count = 10;
            assert_err!(
                Xp::force_genesis_config(
                    RuntimeOrigin::signed(ALICE),
                    ForceGenesisConfig::PulseFactor {
                        threshold,
                        per_count
                    }
                ),
                DispatchError::BadOrigin
            );
        });
    }

    #[test]
    fn call_success() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            Pallet::new_xp(&BOB, &XP_BETA);

            let call = Box::new(Call::Xp(crate::Call::handover {
                xp_id: XP_ALPHA,
                new_owner: BOB,
            }));
            assert_ok!(Pallet::is_owner(&ALICE, &XP_ALPHA));
            System::set_block_number(2);
            assert_ok!(Xp::call(RuntimeOrigin::signed(ALICE), XP_ALPHA, call));
            assert_err!(Pallet::is_owner(&ALICE, &XP_ALPHA), Error::InvalidXpOwner);
            assert_ok!(Pallet::is_owner(&BOB, &XP_ALPHA));
            System::assert_last_event(
                Event::XpOwner {
                    id: XP_ALPHA,
                    owner: BOB,
                }
                .into(),
            );
        });
    }

    #[test]
    fn call_fail_invalid_owner() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            Pallet::new_xp(&BOB, &XP_BETA);

            let call = Box::new(Call::Xp(crate::Call::handover {
                xp_id: XP_ALPHA,
                new_owner: BOB,
            }));
            assert_ok!(Pallet::is_owner(&ALICE, &XP_ALPHA));
            assert_err!(
                Xp::call(RuntimeOrigin::signed(ALICE), XP_BETA, call),
                Error::InvalidXpOwner
            );
        });
    }

    #[test]
    fn call_fail_bad_origin() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            Pallet::new_xp(&BOB, &XP_BETA);

            let call = Box::new(Call::Xp(crate::Call::handover {
                xp_id: XP_ALPHA,
                new_owner: BOB,
            }));
            assert_ok!(Pallet::is_owner(&ALICE, &XP_ALPHA));
            assert_err!(
                Xp::call(RuntimeOrigin::root(), XP_ALPHA, call),
                DispatchError::BadOrigin
            );
        });
    }
}
