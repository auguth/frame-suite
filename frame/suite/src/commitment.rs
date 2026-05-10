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
// ``````````````````````````````` COMMITMENT SUITE ``````````````````````````````
// ===============================================================================

//! A composable framework for expressing financial intent as **immutable commitments**.
//!
//! ## Overview
//!
//! This module defines a comprehensive set of traits for building financial
//! commitment systems that are **semantic, composable, transparent, and consistent**.
//!
//! At its core, a commitment represents **bonding value to a specific digest under a reason**.
//!
//! Instead of treating value as passive balance, this framework models value as
//! something actively **bound with purpose and context**.
//!
//! Every commitment binds an asset to:
//! - a **Reason** - why the value is committed
//! - a **Digest** - the exact terms or context the value is bonded to
//!
//! Together, they form a **verifiable, immutable agreement**.
//!
//! Commitments can carry meaning through variants (e.g., long/short,
//! for/against, positive/negative), support grouping via indexes and pools,
//! and allow real-time inspection of state.
//!
//! The **reason provider (runtime logic)** governs how committed value evolves:
//! it can update digest-level values, effectively managing all bonded funds
//! under that reason. Individual commitments are **resolved lazily**, meaning
//! adjustments are realized only when commitments are settled.
//!
//! ## Mental Model
//!
//! A commitment can be understood as:
//!
//! "Bond this value, for this purpose, to this exact context."
//!
//! - Reason provides intent (staking, escrow, trade, vote)
//! - Digest defines the binding target (hash, identifier, agreement)
//! - Asset represents the value being bonded
//!
//! This abstraction unifies patterns across financial systems such as
//! staking, escrow, betting, trading, and governance.
//!
//! ## Trait Architecture
//!
//! The framework is layered:
//!
//! ### Foundation
//! - [`Commitment`] - atomic bonding and lifecycle of commitments
//! - [`InspectAsset`] - available funds introspection
//! - [`DigestModel`] - digest classification
//!
//! ### Grouping
//! - [`CommitIndex`] - a single commitment distributed across multiple digests
//!   under a reason, as if placed individually
//!
//! - [`CommitPool`] - manager-controlled allocation of bonded funds across
//!   multiple digests under a reason, without custodial ownership
//!
//! ### Semantics
//! - [`CommitVariant`] - directional meaning (long/short, for/against)
//! - [`IndexVariant`] - variants applied to indexed commitments
//! - [`PoolVariant`] - variants applied to pooled commitments
//!
//! These traits compose into a layered system:
//! - Base layer: [`Commitment`], [`InspectAsset`], [`DigestModel`]
//! - Grouping layer: [`CommitIndex`], [`CommitPool`]
//! - Variant layer: [`CommitVariant`], [`IndexVariant`], [`PoolVariant`]
//!
//! ## Core Properties
//!
//! - **Atomic** - bonded commitments cannot be partially altered  
//! - **Immutable** - [`Commitment::Digest`] and reason define permanent binding  
//! - **Value-bound** - always tied to quantifiable assets  
//! - **Composable** - higher-level abstractions reuse the same rules  
//! - **Inspectable** - real-time bonded state is always queryable  
//!
//! ## Why This Exists
//!
//! Many financial systems repeatedly implement the same primitives:
//! - locking or bonding funds  
//! - enforcing rules  
//! - tracking intent  
//! - preventing duplication  
//!
//! This framework provides a unified abstraction to express all of them
//! consistently and safely.
//!
//! ## Invariants
//!
//! - One commitment per `(proprietor, reason)`  
//! - Commitments are immutable (value may only increase via
//!   [`Commitment::raise_commit`])  
//! - [`Commitment::Digest`] defines the binding target and scope  
//! - State reflects current values via [`Commitment::get_digest_value`],
//!   not historical deposits  
//!
//! ## Use Cases
//!
//! This framework is applicable across domains such as:
//!
//! - Financial systems (bets, trades, hedges)  
//! - Prediction markets (affirmative/negative stakes)  
//! - Voting protocols (for/against commitments)  
//! - Portfolio management (indexed and pooled commitments)  
//! - Escrow and contract systems  
//!
//! ## Summary
//!
//! **Commitment = bonding value to a digest under a reason**
//!
//! This module transforms raw assets into structured, meaningful agreements
//! that can be composed into complex financial systems.

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- Core ---
use core::cmp::Ordering;

// --- Local crate imports ---
use crate::{
    base::{
        Asset, Countable, Delimited, Elastic, Keyed, Percentage, RuntimeEnum, RuntimeError,
        Storable,
    },
    misc::{Directive, Extent},
};

// --- FRAME Support ----
use frame_support::ensure;

// --- Substrate primitives ---
use sp_runtime::{
    traits::{Saturating, Zero},
    DispatchError, DispatchResult, Vec,
};

// ===============================================================================
// ```````````````````````````````` INSPECT ASSET ````````````````````````````````
// ===============================================================================

/// A trait for inspecting the total available funds of an proprietor.
///
/// This allows querying all funds that are available to a proprietor,
/// including liquid balances and any amounts held for specific purposes
/// that the system considers available.
///
/// In this trait's context, "available funds" means those balances
/// that can be used for operations in context.
///
/// Generics:
/// - **Proprietor** - the entity that owns the asset and can make commitments.
pub trait InspectAsset<Proprietor> {
    /// Representing the Quantifiable Asset Type.
    type Asset: Asset;

    /// Retrieves the total available funds for a given proprietor.
    ///
    /// This must include all balances that can be utilized for the
    /// implementation purposes.
    ///
    /// ## Returns
    /// - [`Self::Asset`] containing the total available funds for the proprietor
    fn available_funds(who: &Proprietor) -> Self::Asset;
}

// ===============================================================================
// ````````````````````````````````` DIGEST MODEL ````````````````````````````````
// ===============================================================================

/// A trait for determining the model variant of a digest.
///
/// In this system, a "digest" represents a compact identifier for
/// a commitment or resource.
///
/// Since all digests share a common base type, this trait provides a
/// way to classify or wrap them into distinct model variants
/// (e.g., Direct, Index, Pools) to improve clarity and enforce type safety.
pub trait DigestModel<Proprietor>: Commitment<Proprietor> {
    /// Wraps [`Commitment::Digest`] to reduce ambiguity.
    type Model: Delimited + RuntimeEnum + Storable;

    /// Determines the appropriate model variant for the given digest and reason.
    ///
    /// ## Returns
    /// - `Ok(Model)` if the digest is recognized.
    /// - `Err(DispatchError)` if the digest cannot be determined.
    fn determine_digest(
        digest: &Self::Digest,
        reason: &Self::Reason,
    ) -> Result<Self::Model, DispatchError>;
}

// ===============================================================================
// `````````````````````````````` COMMITMENT ERRORS ``````````````````````````````
// ===============================================================================

/// Commitment-related error type used in trait defaults.
pub enum CommitError {
    /// Insufficient funds to perform the requested
    /// commitment operation.
    InsufficientFunds,

    /// A commitment already exists for the given proprietor
    /// and commitment reason.
    CommitAlreadyExists,

    /// Minting is not permitted under the current constraints
    /// or exceeds allowed limits.
    MintingOffLimits,

    /// Reaping (burning) is not permitted under the current
    /// constraints or exceeds allowed limits.
    ReapingOffLimits,

    /// Placing a new commitment is not permitted under the current constraints
    /// or exceeds allowed limits.
    PlacingOffLimits,

    /// Increasing (raising) an existing commitment is not permitted under the
    /// current constraints or exceeds allowed limits.
    RaisingOffLimits,
}

/// A trait for mapping **domain-level Commitment errors** into
/// **caller- or pallet-specific error types**.
///
/// This trait acts as a bridge between the generic, FRAME-agnostic
/// [`CommitError`] enum and the concrete error type expected by the
/// execution context.
pub trait CommitErrorHandler {
    /// Concrete error type produced by the handler.
    ///
    /// Implements conversion to [`DispatchError`].
    type Error: RuntimeError;

    /// Converts a generic [`CommitError`] into the handler's
    /// concrete error type which implements `Into<DispatchError>`.
    ///
    /// This function centralizes error translation logic and ensures
    /// that all balance-related failures are surfaced consistently
    /// according to the caller's error domain.
    fn from_commit_error(e: CommitError) -> Self::Error;
}

// ===============================================================================
// `````````````````````````````````` COMMITMENT `````````````````````````````````
// ===============================================================================

/// Represents an **atomic, immutable financial agreement** between parties.
///
/// It is anchored in:
/// - a **Reason** (category/purpose),
/// - a **Digest** (commitment context),
/// - and the act of committing a value.
///
/// ## Key Concepts
///
/// - **Commitment**: Locking or assigning an asset under agreed terms.
/// - **Reason**: Purpose/category (e.g., staking, escrow, collateral,
/// investment, betting).
/// - **Digest**: Immutable reference to the commitment's context or terms
/// (often a cryptographic hash or a contextual identifier).
/// - **Proprietor**: Entity responsible for holding/committing the asset.
/// - **Asset**: Value being committed.
///
/// ## Why Commitment?
///
/// Many financial systems - staking, escrow, lending, betting, auctions - share
/// a common pattern: one party proposes terms (digest), another commits a value
/// under those terms, and both agree on the purpose (reason).
///
/// Without a unifying abstraction, each system must reimplement logic for:
/// - Locking assets
/// - Ensuring immutability of terms
/// - Preventing double commitments
/// - Categorizing commitments by purpose
///
/// The `Commitment` trait standardizes this pattern, enabling:
/// - **Consistency**: Same behavior across different financial constructs.
/// - **Composability**: Higher-level constructs (pools, indexes, markets) reuse
/// the same rules.
/// - **Auditability**: Commitments can be inspected and verified.
///
/// ## Properties
///
/// - **Atomic**: Commitments cannot be partially altered.
/// - **Immutable**: Digest + Reason permanently define the commitment.
/// - **Category-driven**: Reason defines interpretation.
/// - **Value-based**: Always tied to a quantifiable asset.
///
/// ## Real-World Analogies
///
/// | Use Case       | Reason             | Digest              | Commitment    |
/// |----------------|--------------------|---------------------|---------------|
/// | Stake          | `"staking"`        | Conditions hash     | Staked amount |
/// | Escrow         | `"escrow"`         | Contract terms hash | Locked funds  |
/// | Collateral     | `"loan collateral"`| Loan agreement hash | Pledged asset |
/// | Betting        | `"bet"`            | Bet terms           | Wagered asset |
/// | Market position| `"market trade"`   | Order terms         | Locked funds  |
/// | Auction        | `"auction bid"`    | Bid terms           | Bid amount    |
///
///
/// ## Examples
///
/// - **Staking**: Alice stakes 100 tokens for reason `"staking"` with digest `"ant_pool_hash"`.
///   - Proprietor: Alice
///   - Reason: `"staking"`
///   - Digest: `"ant_pool_hash"`
///   - Asset: 100 tokens.
///
/// - **Betting**: Bob places a bet of 50 tokens for reason `"bet_games"` with digest
/// `"nfl_betting"`.
///   - Locks Bob's stake until event outcome.
///
/// - **Escrow**: Carol commits 1000 tokens into escrow for reason `"escrows"` with
/// digest `"freelance"`.
///   - Funds locked until conditions are fulfilled.
///
/// - **Market Order**: Dave places a buy order worth 500 tokens for reason
/// `"market_orders"` with digest `"cryptocurrencies"`.
///   - Funds locked until order executes or cancels.
///
/// ## Invariants
///
/// - Proprietor can commit to **only one digest per reason**.
/// - Commitments are **atomic** and **immutable**.
/// - Commitment value may increase via [`Commitment::raise_commit`] but cannot
/// be decreased arbitrarily.
/// - Commitment values are dynamic, reflecting real-time state.
/// - Digest values may be updated by the reason provider via
/// [`Commitment::set_digest_value`] and must propagate to all related commitments.
///
/// **Summary:**  
/// Commitment = locked asset,  
/// Reason = purpose/category,  
/// Digest = agreement terms hash,  
/// Act of commitment = immutable financial agreement.
///
/// Generics:
/// - **Proprietor** - the entity that owns the asset and can make commitments.
pub trait Commitment<Proprietor>: InspectAsset<Proprietor> + CommitErrorHandler {
    /// The source used to generate the digest.
    type DigestSource: Elastic;

    /// The immutable substance proposed by the first party - the "sealed deed content".
    /// It represents the details of the deed or offer that expects a commitment.
    ///
    /// Digest is the core of a commitment: it proves what the commitment is tied to,
    /// and ensures it is fixed and immutable.
    type Digest: Keyed;

    /// The category or type of the commitment.
    /// Provides meaning to the digest by placing it in context.
    type Reason: Elastic + Storable + RuntimeEnum;

    /// Represents the qualitative configuration of a commit operation.
    ///
    /// Encodes behavioral characteristics such as precision and fortitude,
    /// influencing how commitments are evaluated and executed. (e.g. exact
    /// vs approximate, polite vs forceful execution).
    ///
    /// Default is provided if in case no preference is provided.
    type Intent: Delimited + Directive + Default;

    /// Provides optional advisory bounds for commitment and digest operations.
    ///
    /// These limits act as an additional validation layer on top of core
    /// invariants, helping prevent premature or extreme values without
    /// affecting correctness.
    ///
    /// Uses [`Extent`] to express minimum, maximum, or optimal values.
    type Limits: Extent<Scalar = Self::Asset>;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` CHECKERS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Checks whether a commitment exists for the given proprietor and reason.
    ///
    /// ### Returns
    /// - `Ok(())` if the commitment exists
    /// - `Err(DispatchError)` if no commitment is found
    fn commit_exists(who: &Proprietor, reason: &Self::Reason) -> DispatchResult;

    /// Checks whether a specific digest exists for the given reason.
    ///
    /// ### Returns
    /// - `Ok(())` if the digest exists
    /// - `Err(DispatchError)` if the digest is not found
    fn digest_exists(reason: &Self::Reason, digest: &Self::Digest) -> DispatchResult;

    /// Validates whether a new commitment can be placed.
    ///
    /// Ensures that no existing commitment exists for the given reason (enforcing
    /// the "one digest per reason" invariant) and that the proprietor has sufficient
    /// available funds to cover the commitment value.
    ///
    /// ## Returns
    /// - `Ok(())` if validation succeeds
    /// - `Err(DispatchError)` if a commitment already exists or insufficient funds are available
    fn can_place_commit(
        who: &Proprietor,
        reason: &Self::Reason,
        digest: &Self::Digest,
        value: Self::Asset,
        qualifier: &Self::Intent,
    ) -> DispatchResult {
        ensure!(
            Self::commit_exists(who, reason).is_err(),
            Self::from_commit_error(CommitError::CommitAlreadyExists)
        );
        let max = Self::available_funds(who);
        ensure!(
            max >= value,
            Self::from_commit_error(CommitError::InsufficientFunds)
        );
        let limits = Self::place_commit_limits(who, reason, digest, qualifier)?;
        ensure!(
            limits.contains(value),
            Self::from_commit_error(CommitError::PlacingOffLimits)
        );
        Ok(())
    }

    /// Validates whether an existing commitment can be increased.
    ///
    /// Ensures that a commitment for the given reason already exists and that
    /// the proprietor has sufficient available funds to increase the commitment
    /// by the specified value.
    ///
    /// ## Returns
    /// - `Ok(())` if validation succeeds
    /// - `Err(DispatchError)` if any of the condition fails.
    fn can_raise_commit(
        who: &Proprietor,
        reason: &Self::Reason,
        value: Self::Asset,
        qualifier: &Self::Intent,
    ) -> DispatchResult {
        Self::commit_exists(who, reason)?;
        let max = Self::available_funds(who);
        ensure!(
            max >= value,
            Self::from_commit_error(CommitError::InsufficientFunds)
        );
        let limits = Self::raise_commit_limits(who, reason, qualifier)?;
        ensure!(
            limits.contains(value),
            Self::from_commit_error(CommitError::RaisingOffLimits)
        );
        Ok(())
    }

    /// Validates whether an existing commitment can be resolved.
    ///
    /// Ensures that a commitment for the given reason exists before
    /// allowing resolution/closing.
    ///
    /// ## Returns
    /// - `Ok(())` if validation succeeds
    /// - `Err(DispatchError)` if no such commitment exists
    fn can_resolve_commit(who: &Proprietor, reason: &Self::Reason) -> DispatchResult {
        Self::commit_exists(who, reason)?;
        Ok(())
    }

    /// Validates whether a digest's value can be set or updated.
    ///
    /// Ensures that the specified digest exists under the given reason,
    /// and that the intended value update respects advisory limits
    /// (mint or reap depending on direction).
    ///
    /// The `qualifier` may influence how limits are interpreted.
    ///
    /// ## Returns
    /// - `Ok(())` if validation succeeds
    /// - `Err(DispatchError)` if the digest does not exist or limits are violated
    fn can_set_digest_value(
        reason: &Self::Reason,
        digest: &Self::Digest,
        value: Self::Asset,
        qualifier: &Self::Intent,
    ) -> DispatchResult {
        // Get current value
        let current = Self::get_digest_value(reason, digest)?;
        match current.cmp(&value) {
            Ordering::Less => {
                // Mint path (increase)
                let limits = Self::digest_mint_limits(digest, reason, qualifier)?;
                let mintable = value.saturating_sub(current);
                ensure!(
                    limits.contains(mintable),
                    Self::from_commit_error(CommitError::MintingOffLimits)
                )
            }
            Ordering::Greater => {
                // Reap path (decrease)
                let limits = Self::digest_reap_limits(digest, reason, qualifier)?;
                let reapable = current.saturating_sub(value);
                ensure!(
                    limits.contains(reapable),
                    Self::from_commit_error(CommitError::ReapingOffLimits)
                )
            }
            Ordering::Equal => {
                // No-op, always valid
            }
        }
        Ok(())
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` GETTERS ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Retrieves the digest tied to a proprietor's commitment for the given reason.
    ///
    /// Since the system enforces only one digest per reason, its expected to be
    /// only a single digest or none.
    ///
    /// ### Returns
    /// - `Ok(Digest)` containing the commitment's digest
    /// - `Err(DispatchError)` if no commitment exists for the reason
    fn get_commit_digest(
        who: &Proprietor,
        reason: &Self::Reason,
    ) -> Result<Self::Digest, DispatchError>;

    /// Retrieves the total aggregated value for all commitments under the given reason.
    ///
    /// ### Returns
    /// - `Self::Asset` containing the total value across all commitments for the reason
    fn get_total_value(reason: &Self::Reason) -> Self::Asset;

    /// Retrieves the current real-time value of a proprietor's commitment for the given reason.
    ///
    /// Returns the live committed value at the moment of calling, reflecting any
    /// changes to the underlying digest value since the commitment was initially
    /// placed. This is not a historical value but the actual current state, as
    /// digest values can be updated via [`set_digest_value`](Self::set_digest_value).
    ///
    /// ### Returns
    /// - `Ok(Asset)` containing the current commitment value
    /// - `Err(DispatchError)` if the commitment does not exist
    fn get_commit_value(
        who: &Proprietor,
        reason: &Self::Reason,
    ) -> Result<Self::Asset, DispatchError>;

    /// Retrieves the current aggregated value for the given digest under the specified reason.
    ///
    /// Returns the real-time sum of all commitments tied to the digest.
    /// Useful for evaluating the full scope of commitments to a specific digest.
    ///
    /// ### Returns
    /// - `Ok(Asset)` containing the aggregated digest value
    /// - `Err(DispatchError)` if the digest does not exist or lookup fails
    fn get_digest_value(
        reason: &Self::Reason,
        digest: &Self::Digest,
    ) -> Result<Self::Asset, DispatchError>;

    /// Provides advisory bounds for increasing (minting) a digest's value.
    ///
    /// Acts as an optional guard to prevent abrupt or excessive growth,
    /// complementing core digest update logic.
    fn digest_mint_limits(
        _digest: &Self::Digest,
        _reason: &Self::Reason,
        _qualifier: &Self::Intent,
    ) -> Result<Self::Limits, DispatchError> {
        // By default, this returns a baseline limit,
        // representing a static or fallback bound when no dynamic limits are applied.
        //
        // Implementors may override this to provide context-specific limits.
        Ok(Self::Limits::none())
    }

    /// Provides advisory bounds for placing a new commitment.
    ///
    /// Acts as an optional pre-validation layer to prevent premature or
    /// undesirable commits by constraining value ranges.
    fn place_commit_limits(
        _who: &Proprietor,
        _reason: &Self::Reason,
        _digest: &Self::Digest,
        _qualifier: &Self::Intent,
    ) -> Result<Self::Limits, DispatchError> {
        // By default, this returns an unbounded extent,
        // meaning no additional constraints are applied unless overridden.
        //
        // Implementors may override this to provide context-specific limits.
        Ok(Self::Limits::none())
    }

    /// Provides advisory bounds for increasing an existing commitment.
    ///
    /// Acts as an optional guard to prevent excessive or premature raises,
    /// complementing core checks like existence and available funds.
    fn raise_commit_limits(
        _who: &Proprietor,
        _reason: &Self::Reason,
        _qualifier: &Self::Intent,
    ) -> Result<Self::Limits, DispatchError> {
        // By default, this returns an unbounded extent,
        // meaning no additional constraints are applied unless overridden.
        //
        // Implementors may override this to provide context-specific limits.
        Ok(Self::Limits::none())
    }

    /// Provides advisory bounds for decreasing (reaping) a digest's value.
    ///
    /// Acts as an optional guard to prevent aggressive or premature reductions,
    /// complementing core invariants that ensure correctness.
    fn digest_reap_limits(
        _digest: &Self::Digest,
        _reason: &Self::Reason,
        _qualifier: &Self::Intent,
    ) -> Result<Self::Limits, DispatchError> {
        // By default, this returns a baseline limit,
        // representing a static or fallback bound when no dynamic limits are applied.
        // Implementors may override this to provide context-specific limits.
        Ok(Self::Limits::none())
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ````````````````````````````````` CONSTRUCTORS ````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Generates a unique digest from the provided source.
    ///
    /// Creates an immutable identifier that represents the commitment's terms,
    /// ensuring collision-free digest generation across the system.
    ///
    /// ## Returns
    /// - `Ok(Digest)` containing the generated unique digest
    /// - `Err(DispatchError)` if digest generation fails
    fn gen_digest(via: &Self::DigestSource) -> Result<Self::Digest, DispatchError>;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` MUTATORS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Places an immutable commitment for the specified digest and reason.
    ///
    /// Locks the specified value under the given digest and reason, creating
    /// an atomic commitment that cannot be reduced or partially withdrawn.
    ///
    /// The qualifier parameter enforces exactness or best-effort semantics,
    /// while also specifying whether the system must force achieving the
    /// commitment conditions.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the actual value committed to the digest
    /// - `Err(DispatchError)` if placement fails due to insufficient funds or
    /// validation errors
    fn place_commit(
        who: &Proprietor,
        reason: &Self::Reason,
        digest: &Self::Digest,
        value: Self::Asset,
        qualifier: &Self::Intent,
    ) -> Result<Self::Asset, DispatchError>;

    /// Resolves and releases the commitment tied to the specified reason.
    ///
    /// Finalizes the commitment for the given proprietor and reason, releasing
    /// the locked value.
    ///
    /// Proprietors are enforced to commit to a **single digest per reason**,
    /// ensuring unambiguous resolution. Higher-level traits such as [`CommitIndex`]
    /// or [`CommitPool`] can be built to allow indexed or pooled commitments on top of
    /// this invariant.
    ///    
    /// The returned value may differ from the original deposit depending on the reason's
    /// context and any adjustments made during the commitment's lifetime possibly
    /// via [`Commitment::set_digest_value`].
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the fully resolved commitment value
    /// - `Err(DispatchError)` if the commitment does not exist or resolution fails
    fn resolve_commit(
        who: &Proprietor,
        reason: &Self::Reason,
    ) -> Result<Self::Asset, DispatchError>;

    /// Increases the value of an existing commitment.
    ///
    /// Adds the specified value to an existing commitment for the given reason.
    /// This operation can only be performed if a commitment already exists.
    ///
    /// The qualifier parameter control how the increase is applied.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the increment actually added (not the total value)
    /// - `Err(DispatchError)` if the commitment does not exist or if the increase fails
    fn raise_commit(
        who: &Proprietor,
        reason: &Self::Reason,
        value: Self::Asset,
        qualifier: &Self::Intent,
    ) -> Result<Self::Asset, DispatchError>;

    /// Sets or updates the aggregated value of the specified digest.
    ///
    /// Allows the reason provider (implementer) to adjust the digest's value,
    /// which automatically propagates to all individual commitments tied to that
    /// digest.
    ///
    /// The `qualifier` defines how the update is applied (e.g., exact vs best-effort,
    /// forceful vs relaxed), and determines the execution semantics for the direction
    /// of change (increase or decrease relative to the current value).
    ///
    /// ### Returns
    /// - `Ok(Asset)` containing the actual value applied to the digest
    /// - `Err(DispatchError)` if the digest does not exist or update fails
    fn set_digest_value(
        reason: &Self::Reason,
        digest: &Self::Digest,
        value: Self::Asset,
        qualifier: &Self::Intent,
    ) -> Result<Self::Asset, DispatchError>;

    /// Removes a digest if it has no associated commitments.
    ///
    /// Permanently deletes the specified digest from the system if its aggregated
    /// value is zero, freeing resources and cleaning up unused entries.
    /// This is a maintenance operation and it should ensures no commitments exist
    /// for the digest before reaping.
    ///
    /// ### Returns
    /// - `Ok(())` if the digest is successfully removed
    /// - `Err(DispatchError)` if the digest does not exist or still has active commitments
    fn reap_digest(digest: &Self::Digest, reason: &Self::Reason) -> DispatchResult;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````````` HOOKS ````````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Hook called after a commitment is successfully placed.
    ///
    /// Provides an extension point for triggering side-effects such as events,
    /// logging, external state updates, or notifications when a new commitment
    /// is established.
    ///
    /// Default implementation is a no-op.
    fn on_commit_place(
        _who: &Proprietor,
        _reason: &Self::Reason,
        _digest: &Self::Digest,
        _value: Self::Asset,
    ) {
    }

    /// Hook called after a commitment is successfully raised (increased).
    ///
    /// Provides an extension point for triggering side-effects such as recalculations,
    /// notifications, event emissions, or external state synchronization when an
    /// existing commitment's value is increased.
    ///
    /// Default implementation is a no-op.
    fn on_commit_raise(
        _who: &Proprietor,
        _reason: &Self::Reason,
        _digest: &Self::Digest,
        _value: Self::Asset,
    ) {
    }

    /// Hook called after a commitment is successfully resolved.
    ///
    /// Provides an extension point for performing cleanup, distributing rewards or
    /// penalties, audit logging, or triggering any finalization logic when a
    /// commitment is released.
    ///
    /// Default implementation is a no-op.
    fn on_commit_resolve(
        _who: &Proprietor,
        _reason: &Self::Reason,
        _digest: &Self::Digest,
        _value: Self::Asset,
    ) {
    }

    /// Hook called after a digest's value is updated.
    ///
    /// Provides an extension point for triggering recalculation of dependent
    /// commitments, propagating changes to related state, or emitting events
    /// when a digest's aggregated value changes.
    ///
    /// Default implementation is a no-op.
    fn on_digest_update(_digest: &Self::Digest, _reason: &Self::Reason, _value: Self::Asset) {}

    /// Hook called after a digest is reaped (removed).
    ///
    /// Provides an extension point for cleanup tasks, logging, freeing
    /// related resources, or triggering external notifications when a digest
    /// is permanently removed from the system.
    ///
    /// `dust` represents any residual value that was unclaimable and
    /// effectively considered dead.
    ///
    /// Default implementation is a no-op.
    fn on_reap_digest(_digest: &Self::Digest, _reason: &Self::Reason, _dust: Self::Asset) {}
}

// ===============================================================================
// ````````````````````````````````` COMMIT INDEX ````````````````````````````````
// ===============================================================================

/// `CommitIndex` extends [`Commitment`] by providing a **higher-level financial abstraction**
/// that groups multiple commitments under a single "index".
///
/// This enables proprietors to manage related commitments collectively,
/// while preserving the **atomicity** and **immutability** guarantees of `Commitment`.
///
/// ## Use Cases
///
/// - Portfolio management: Aggregate commitments.
/// - Betting pools: Collective tracking and settlement.
/// - Multi-asset contracts: Manage grouped assets with shares.
/// - Escrow pools: Efficient tracking of grouped escrows.
/// - Market positions: Aggregate for easier tracking.
///
/// ## Why `CommitIndex`?
///
/// The base `Commitment` trait enforces:
/// > "One reason -> one digest".
///
/// This is restrictive when:
/// - A proprietor needs to manage **multiple related commitments under one reason**.
/// - Aggregation of commitments is required.
/// - Ownership of grouped commitments must be tracked proportionally.
/// - Trustless, composable structures are desired.
///
/// `CommitIndex` solves this by introducing **indexes**:
/// - An **index digest** references multiple committed digests (entries).
/// - Each entry remains an independent `Commitment`.
/// - The index enables aggregation, share-tracking, and collective management
/// without breaking core commitment rules.
///
/// ## Core Principles
///
/// 1. **Wrapper over commitments**
///    - Index digest groups multiple committed digests.
///    - Entries retain individual commitment properties.
///
/// 2. **Management layer**
///    - Tracks shares and values for each entry.
///    - Aggregates entry values into a single index value.
///
/// 3. **Integrity**
///    - Entries remain independent commitments.
///    - Index creation does not alter entry commitments.
///
/// 4. **Trustless design**
///    - Anyone can interact with indexes without requiring creator consent,
///      provided the share structure allows it.
///
/// ## Examples
///
/// ### Example 1: Portfolio Staking
///
/// Alice stakes multiple digests under a single reason to manage risk:
///
/// | Reason     | Digest        | Value |
/// |------------|---------------|-------|
/// | "staking"  | "digest_a123" | 100   |
/// | "staking"  | "digest_b456" | 200   |
/// | "staking"  | "digest_c789" | 300   |
///
/// Each row is an independent commitment.
/// Alice creates a `CommitIndex`:
/// - Reason = `"staking"`
/// - Digest = `"index_digest_xyz"`
/// - Entries = `[digest_a123, digest_b456, digest_c789]`
/// - Shares = `[1, 2, 3]`
///
/// Total value = `600`, with proportional share tracking.
///
/// ### Example 2: Betting Pool
///
/// Bettors commit value for different bets under the same reason:
///
/// | Reason         | Digest           | Value |
/// |----------------|------------------|-------|
/// | "betting_pool" | "digest_bet101" | 50    |
/// | "betting_pool" | "digest_bet102" | 80    |
///
/// CommitIndex:
/// - Reason = `"betting_pool"`
/// - Digest = `"index_digest_betting"`
/// - Entries = `[digest_bet101, digest_bet102]`
/// - Shares = `[2, 3]`
///
/// Total value = `130`, with proportional share tracking.
///
/// ### Example 3: Market Position Index
///
/// Dave aggregates multiple market positions:
///
/// | Reason            | Digest         | Value |
/// |-------------------|----------------|-------|
/// | "market_positions"| "digest_order1"| 500   |
/// | "market_positions"| "digest_order2"| 300   |
///
/// CommitIndex:
/// - Reason = `"market_positions"`
/// - Digest = `"index_digest_market"`
/// - Entries = `[digest_order1, digest_order2]`
/// - Shares = `[5, 3]`
///
/// Aggregates positions without altering atomic commitments.
///
/// ### Example 4: Escrow Pool
///
/// Multiple escrows under one reason:
///
/// | Reason       | Digest          | Value |
/// |--------------|-----------------|-------|
/// | "escrow_pool"| "digest_escrow1"| 1000  |
/// | "escrow_pool"| "digest_escrow2"| 1500  |
///
/// CommitIndex:
/// - Reason = `"escrow_pool"`
/// - Digest = `"index_digest_escrow"`
/// - Entries = `[digest_escrow1, digest_escrow2]`
/// - Shares = `[1, 1.5]`
///
/// Tracks total escrow commitments (`2500`) and proportional ownership.
///
/// ## Invariants
///
/// - An index digest is a managed wrapper over multiple committed digests.
/// - Each entry is an independent `Commitment`.
/// - Index creation preserves atomicity, immutability, and reason-digest invariants.
/// - Shares define proportional ownership of aggregated commitments.
/// - Must maintain the base-invariant "One Reason -> One Digest"
///
/// Generics:
/// - **Proprietor** - the entity that owns the asset and can make commitments.
pub trait CommitIndex<Proprietor>: Commitment<Proprietor> {
    /// The type representing an index.
    /// This could be a struct containing entries and shares.
    type Index: Elastic + Storable;

    /// The type representing shares for an entry.
    ///
    /// Should be a simple unsigned numeric type.
    type Shares: Countable;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` CHECKERS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Checks whether an index exists for the given reason and index digest.
    ///
    /// ## Returns
    /// - `Ok(())` if the index exists
    /// - `Err(DispatchError)` if the index is not found
    fn index_exists(reason: &Self::Reason, index_of: &Self::Digest) -> DispatchResult;

    /// Checks whether a specific entry exists within the given index.
    ///
    /// Verifies that the specified entry digest is part of the index's
    /// entry list under the given reason.
    ///
    /// ## Returns
    /// - `Ok(())` if the entry exists within the index
    /// - `Err(DispatchError)` if the entry is not found in the index
    fn entry_exists(
        reason: &Self::Reason,
        index_of: &Self::Digest,
        entry_of: &Self::Digest,
    ) -> DispatchResult;

    /// Checks whether any index exists for the given reason.
    ///
    /// Verifies that at least one index has been created under the
    /// specified reason across all proprietors.
    ///
    /// ## Returns
    /// - `Ok(())` if at least one index exists for the reason
    /// - `Err(DispatchError)` if no indexes are found for the reason
    fn has_index(reason: &Self::Reason) -> DispatchResult;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` GETTERS ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Retrieves the complete index structure for the given reason and index digest.
    ///
    /// Returns the full index object containing all entries, shares, and
    /// associated metadata for inspection or processing.
    ///
    /// ## Returns
    /// - `Ok(Index)` containing the complete index structure i.e.,
    /// [`CommitIndex::Index`]
    /// - `Err(DispatchError)` if the index does not exist
    fn get_index(
        reason: &Self::Reason,
        index_of: &Self::Digest,
    ) -> Result<Self::Index, DispatchError>;

    /// Retrieves the real-time aggregated value of the specified index.
    ///
    /// Computes the total value by summing the current values of all entry digests
    /// under the index. Since the supertrait [`Commitment`] allows digest values
    /// to be updated via [`Commitment::set_digest_value`], this reflects the
    /// **live state** rather than historical deposits.
    ///
    /// Ensures each entry's digest value is current before aggregation.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the total aggregated value of the index
    /// - `Err(DispatchError)` if the index does not exist or value computation fails
    fn get_index_value(
        reason: &Self::Reason,
        index_of: &Self::Digest,
    ) -> Result<Self::Asset, DispatchError> {
        Self::index_exists(reason, index_of)?;
        let entries_values = Self::get_entries_value(reason, index_of)?;
        let mut value = Self::Asset::zero();
        for (_, val) in entries_values {
            value = value.saturating_add(val);
        }
        Ok(value)
    }

    /// Retrieves the shares of each entry in the given index.
    ///
    /// Returns a list of entry digests paired with their corresponding shares,
    /// representing each entry's proportional weight within the index.
    ///
    /// ## Returns
    /// - `Ok(Vec<(Self::Digest, Self::Shares)>)` containing entry-share pairs
    /// - `Err(DispatchError)` if the index does not exist or retrieval fails
    fn get_entries_shares(
        reason: &Self::Reason,
        index_of: &Self::Digest,
    ) -> Result<Vec<(Self::Digest, Self::Shares)>, DispatchError>;

    /// Retrieves the real-time values of all entries within the specified index.
    ///
    /// Each entry's value is fetched individually and reflects any changes since
    /// the commitment was created, as the supertrait [`Commitment`] allows digest
    /// values to be updated via [`Commitment::set_digest_value`].
    ///
    /// Returns a list of entry digests paired with their current committed values.
    ///
    /// ## Returns
    /// - `Ok(Vec<(Self::Digest, Self::Asset)>)` containing entry-value pairs
    /// - `Err(DispatchError)` if the index does not exist or value retrieval fails
    fn get_entries_value(
        reason: &Self::Reason,
        index_of: &Self::Digest,
    ) -> Result<Vec<(Self::Digest, Self::Asset)>, DispatchError> {
        Self::index_exists(reason, index_of)?;
        let entries = Self::get_entries_shares(reason, index_of)?;
        let mut vec = Vec::new();
        for (entry_of, _) in entries {
            let value = Self::get_entry_value(reason, index_of, &entry_of)?;
            vec.push((entry_of, value))
        }
        Ok(vec)
    }

    /// Retrieves the real-time committed value of a specific entry within an index.
    ///
    /// Returns the current value for the given entry digest under the specified
    /// reason and index. Since the supertrait [`Commitment`] allows digest values
    /// to be updated via [`Commitment::set_digest_value`], this reflects the
    /// **live state** rather than the original deposit.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the entry's current committed value
    /// - `Err(DispatchError)` if the index or entry does not exist
    fn get_entry_value(
        reason: &Self::Reason,
        index_of: &Self::Digest,
        entry_of: &Self::Digest,
    ) -> Result<Self::Asset, DispatchError>;

    /// Retrieves the real-time values of a proprietor's commitments to all entries
    /// within the specified index.
    ///
    /// Each entry's value is computed individually for the given proprietor and
    /// reflects any changes since commitment, as the supertrait [`Commitment`]
    /// allows digest values to be updated via [`Commitment::set_digest_value`].
    ///
    /// Returns a list of entry digests paired with their current committed values
    /// for the specified proprietor.
    ///
    /// ## Returns
    /// - `Ok(Vec<(Self::Digest, Self::Asset)>)` containing entry-value pairs for the proprietor
    /// - `Err(DispatchError)` if the index does not exist or value retrieval fails
    fn get_entries_value_for(
        who: &Proprietor,
        reason: &Self::Reason,
        index_of: &Self::Digest,
    ) -> Result<Vec<(Self::Digest, Self::Asset)>, DispatchError> {
        Self::index_exists(reason, index_of)?;
        let entries = Self::get_entries_shares(reason, index_of)?;
        let mut vec = Vec::new();
        for (entry_of, _) in entries {
            let value = Self::get_entry_value_for(who, reason, index_of, &entry_of)?;
            vec.push((entry_of, value))
        }
        Ok(vec)
    }

    /// Retrieves the real-time value of a proprietor's commitment to a specific
    /// entry within an index.
    ///
    /// Returns the live committed value for the given proprietor and entry digest.
    /// Since the supertrait [`Commitment`] allows digest values to be updated via
    /// [`Commitment::set_digest_value`], this reflects the **current state** rather
    /// than the deposited total.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the proprietor's current commitment to the entry
    /// - `Err(DispatchError)` if the index or entry does not exist
    fn get_entry_value_for(
        who: &Proprietor,
        reason: &Self::Reason,
        index_of: &Self::Digest,
        entry_of: &Self::Digest,
    ) -> Result<Self::Asset, DispatchError>;

    /// Retrieves the real-time total value of a proprietor's commitment to an index.
    ///
    /// Aggregates the live committed values of all entry digests within the index
    /// for the given proprietor. Since the supertrait [`Commitment`] allows digest
    /// values to be updated via [`Commitment::set_digest_value`], this reflects
    /// the **current total** rather than historical deposits.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the proprietor's total commitment to the index
    /// - `Err(DispatchError)` if the index does not exist or computation fails
    fn get_index_value_for(
        who: &Proprietor,
        reason: &Self::Reason,
        index_of: &Self::Digest,
    ) -> Result<Self::Asset, DispatchError> {
        Self::index_exists(reason, index_of)?;
        let mut value = Self::Asset::zero();
        let entries = Self::get_entries_value_for(who, reason, index_of)?;
        for (_, val) in entries {
            value = value.saturating_add(val)
        }
        Ok(value)
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ````````````````````````````````` CONSTRUCTORS ````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Generates a unique digest for the given index object.
    ///
    /// Creates a distinct identifier derived from the proprietor, reason, and
    /// index structure (including entries and shares). The generated digest
    /// must be collision-resistant and unique across all indexes of reason.
    ///
    /// ### Returns
    /// - `Ok(Digest)` containing the generated unique index digest
    /// - `Err(DispatchError)` if digest generation fails due to invalid inputs
    fn gen_index_digest(
        from: &Proprietor,
        reason: &Self::Reason,
        index: &Self::Index,
    ) -> Result<Self::Digest, DispatchError>;

    /// Prepares an index object from the provided entry data.
    ///
    /// Constructs a valid index structure from a list of `(Digest, Shares)` pairs,
    /// where each pair represents:
    /// - **Digest**: The unique identifier of an entry within the index
    /// - **Shares**: The proportional weight or ownership of that entry
    ///
    /// This method:
    /// - Validates entry data for consistency and integrity
    /// - Ensures no duplicate digests exist
    /// - Rejects entries with nil/empty digests or zero shares
    /// - Creates an immutable, atomic index object
    ///
    /// ## Returns
    /// - `Ok(Index)` containing the prepared index structure
    /// - `Err(DispatchError)` if preparation fails due to invalid data or validation errors
    fn prepare_index(
        who: &Proprietor,
        reason: &Self::Reason,
        entries: &[(Self::Digest, Self::Shares)],
    ) -> Result<Self::Index, DispatchError>;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` MUTATORS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Binds the prepared index to the specified digest under the given reason.
    ///
    /// Associates the provided digest with the index structure, making it
    /// queryable and usable within the commitment system. The index object
    /// should be safely prepared via [`CommitIndex::prepare_index`] before
    /// calling this method.
    ///
    /// This operation ensures:
    /// - The digest uniquely identifies the index
    /// - All entries and shares are preserved with integrity
    /// - The index conforms to [`CommitIndex`] invariants
    ///
    /// ## Returns
    /// - `Ok(())` if the index is successfully set
    /// - `Err(DispatchError)` if the operation fails due to invalid data or conflicts
    fn set_index(
        who: &Proprietor,
        reason: &Self::Reason,
        index: &Self::Index,
        digest: &Self::Digest,
    ) -> DispatchResult;

    /// Updates the shares of a single entry within an existing index.
    ///
    /// Since indexes are immutable once created, this method internally:
    /// 1. Prepares a new index with the updated share for the specified entry
    /// 2. Generates a new digest for the modified index
    /// 3. Returns the new index digest
    ///
    /// Since no explicit entry-removal methods are provided, this method may be
    /// used to remove an entry when its share is set to zero.
    ///
    /// By invariant, commitment indexes must not contain entries with zero shares.
    ///
    /// For updating multiple entries, use [`CommitIndex::prepare_index`] and
    /// [`CommitIndex::set_index`] to create a completely new index structure.
    ///
    /// ## Returns
    /// - `Ok(Digest)` containing the new index digest after share update
    /// - `Err(DispatchError)` if the index or entry does not exist, or update fails
    fn set_entry_shares(
        who: &Proprietor,
        reason: &Self::Reason,
        index_of: &Self::Digest,
        entry_of: &Self::Digest,
        shares: Self::Shares,
    ) -> Result<Self::Digest, DispatchError>;

    /// Removes an index if it contains no active commitments.
    ///
    /// Permanently deletes the specified index digest under the given reason,
    /// freeing associated resources. Indexes can only be reaped when they have
    /// no committed entries or remaining balances.
    ///
    /// ## Returns
    /// - `Ok(())` if the index is successfully removed
    /// - `Err(DispatchError)` if the index does not exist or contains active commitments
    fn reap_index(reason: &Self::Reason, index_of: &Self::Digest) -> DispatchResult;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````````` HOOKS ````````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Hook called after an index is created and its digest is set by a proprietor.
    ///
    /// Provides an extension point for triggering side-effects such as events,
    /// logging, recalculations, or external state updates when a new index is
    /// established.
    ///
    /// Default implementation is a no-op.
    fn on_set_index(
        _who: &Proprietor,
        _index_of: &Self::Digest,
        _reason: &Self::Reason,
        _index: &Self::Index,
    ) {
    }

    /// Hook called after an index is reaped (removed).
    ///
    /// Provides an extension point for cleanup tasks, logging, freeing
    /// related resources, or triggering external notifications when an index
    /// is permanently removed from the system.
    ///
    /// `dust` represents any residual value that was unclaimable and
    /// effectively considered dead.
    ///
    /// Default implementation is a no-op.
    fn on_reap_index(_index_of: &Self::Digest, _reason: &Self::Reason, _dust: Self::Asset) {}
}

// ===============================================================================
// ````````````````````````````````` COMMIT POOL `````````````````````````````````
// ===============================================================================

/// `CommitPool` extends [`Commitment`] and [`CommitIndex`] to provide a **managed,
/// dynamic commitment abstraction**.
///
/// It enables a trusted manager to actively manage where deposited assets are committed
/// - effectively controlling how and where value is allocated across different digests
/// within a pool.
///
/// While `CommitIndex` aggregates multiple commitments under a single digest,
/// `CommitPool` introduces **live management**:
/// - Proprietors deposit assets into a managed pool.  
/// - The pool manager determines how these assets are allocated across underlying
///   commitments to optimize yield, diversify risk, or execute specific strategies.  
/// - Proprietors trust the manager to act within agreed rules and immutable commission terms.
///
/// In short, Pool digests are stable identities; index digests are content hashes.
///
/// ## Purpose
///
/// Pools are useful for:
/// - Managed staking or investment
/// - Delegated liquidity provision
/// - Escrow pools with oversight
/// - Collective asset management with dynamic strategy
///
/// They preserve the atomicity and immutability of individual commitments while enabling
/// flexible allocation.
///
/// ## Core Principles
///
/// 1. **Managed aggregation**
///    - Pools group commitments under a single digest for a given reason.
///    - Slots remain immutable commitments.
///
/// 2. **Dynamic shares**
///    - Managers adjust shares to change exposure or strategy.
///    - Share changes require releasing and recovering the pool.
///
/// 3. **Semi-trusted management**
///    - Managers cannot withdraw deposits directly.
///    - Operations are restricted by pool rules and fixed commission rates.
///
/// 4. **Immutable commission**
///    - Commission rates are fixed at pool creation for transparency and trust.
///
/// ## Examples
///
/// ### Example 1: Managed Staking Pool
///
/// Manager Dave creates a managed pool:
/// - Reason = `"managed_staking"`
/// - Digest = `"dave_pool"`
/// - Entries = `[digest_a123, digest_b456, digest_c789]`
/// - Shares = `[1, 2, 3]`
/// - Commission = `2%`
///
/// Alice, Bob, and Carol deposit tokens into a staking pool managed by Dave:
///
/// | Proprietor | Reason    | Digest        | Value |
/// |------------|-----------|---------------|-------|
/// | Alice      | staking   | dave_pool     | 100   |
/// | Bob        | staking   | dave_pool     | 200   |
/// | Carol      | staking   | dave_pool     | 300   |
///
/// The pool holds a total value of `600`, managed dynamically without altering deposits.  
///
/// Dave can rebalance the pool or add new staking positions (new entry digests)  
/// as opportunities arise, while depositors retain proportional ownership.
///
/// This makes `CommitPool` distinct from `CommitIndex`,  
/// which has **static entries and immutable share structures**.
///
/// #### Dynamic Reallocation and Expansion
///
/// After creation, the manager can **update both shares and entries**:
///
/// - Before:
///   - Entries = `[digest_a123, digest_b456, digest_c789]`
///   - Shares  = `[1, 2, 3]`
///
/// - After:
///   - Entries = `[digest_a123, digest_b456, digest_c789, digest_d999]`
///   - Shares  = `[2, 1, 4, 2]`
///
/// This allows the pool to grow by adding new slots (digests)  
/// and to rebalance proportional exposure.  
/// Proprietors can view current allocations at any time,  
/// but allocations and entries may change as part of active management.
///
/// ### Example 2: Managed Liquidity Pool
///
/// Manager creates:
/// - Reason = `"liquidity"`
/// - Digest = `"pool_digest_lp"`
/// - Entries = `[digest_lp_001, digest_lp_002]`
/// - Shares = `[5, 7]`
/// - Commission = `1%`
///
/// The pool manager may later add new liquidity positions (e.g., `digest_lp_003`)  
/// or adjust existing shares to respond to market demand.  
/// Depositors continue to share in the aggregated performance of the evolving pool.
///
/// ### Example 3: Escrow Pool with Commission
///
/// Carol and Bob deposit into a managed escrow pool:
///
/// | Proprietor | Reason        | Digest              | Value |
/// |------------|---------------|---------------------|-------|
/// | Carol      | escrow        | pool_digest_escrow  | 1000  |
/// | Bob        | escrow        | pool_digest_escrow  | 1500  |
///
/// Manager creates:
/// - Reason = `"escrow"`
/// - Digest = `"pool_digest_escrow"`
/// - Shares = `[1, 1.5]`
/// - Commission = `0.5%`
///
/// The manager can later add new escrow digests or rebalance shares  
/// to adjust allocations, while commission is paid automatically upon resolution.
///
/// ### Example 4: Delegated Investment Pool
///
/// Investors deposit:
///
/// | Proprietor | Reason           | Digest         | Value |
/// |------------|------------------|----------------|-------|
/// | A          | investment       | inv_pool       | 1000  |
/// | B          | investment       | inv_pool       | 2000  |
///
/// Manager creates:
/// - Reason = `"investment"`
/// - Digest = `"inv_pool"`
/// - Shares = `[1, 2]`
/// - Commission = `1.5%`
///
/// After creation:
/// - The manager may add new investment digests or update existing ones.  
/// - Proprietors can see current allocations, but these can change over time.  
///
/// ## Invariants
///
/// - A pool digest wraps multiple committed entries.
/// - Pools are created from indexes.
/// - Mutations require full release and recovering.
/// - Managers cannot withdraw deposits, only adjust shares.
/// - Commission rate is fixed at creation.
/// - Commission is paid upon commitment resolution.
/// - Share adjustments must preserve proportional commitments.
///
/// ## Summary
///
/// `CommitPool` enables managed aggregation of commitments with dynamic share adjustment,
/// preserving safety, immutability, and fixed commission rates.
///
/// Ideal for managed staking, liquidity provision, delegated investment, escrow pools,
/// and collective asset management where live management is required.
///
/// Generics:
/// - **Proprietor** - the entity that owns the asset and can make commitments.
pub trait CommitPool<Proprietor>: Commitment<Proprietor> + CommitIndex<Proprietor> {
    /// The type representing a managed pool.
    ///
    /// Typically a struct that includes:
    /// - The pool's manager (of type `Proprietor`)
    /// - A list of slot digests and their shares
    /// - The commission rate
    /// - Metadata
    type Pool: Elastic + Storable;

    /// The type representing a pool's commission.
    ///
    /// Can be a numeric percentage, ratio, or more complex type
    /// encoding the manager's commission or performance fee model.
    type Commission: Percentage;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` CHECKERS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Checks whether a pool exists for the given reason and pool digest.
    ///
    /// ## Returns
    /// - `Ok(())` if the pool exists
    /// - `Err(DispatchError)` if the pool is not found
    fn pool_exists(reason: &Self::Reason, pool_of: &Self::Digest) -> DispatchResult;

    /// Checks whether a specific slot exists within the given pool.
    ///
    /// Verifies that the specified slot digest is part of the pool's
    /// slot list under the given reason.
    ///
    /// ## Returns
    /// - `Ok(())` if the slot exists within the pool
    /// - `Err(DispatchError)` if the slot is not found in the pool
    fn slot_exists(
        reason: &Self::Reason,
        pool_of: &Self::Digest,
        slot_of: &Self::Digest,
    ) -> DispatchResult;

    /// Checks whether any pool exists for the given reason.
    ///
    /// Verifies that at least one pool has been created under the
    /// specified reason across all managers.
    ///
    /// ## Returns
    /// - `Ok(())` if at least one pool exists for the reason
    /// - `Err(DispatchError)` if no pools are found for the reason
    fn has_pool(reason: &Self::Reason) -> DispatchResult;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` GETTERS ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Retrieves the manager (controller) of the specified pool.
    ///
    /// Returns the proprietor who is responsible for managing the pool's
    /// allocations, slot configurations, and commission distribution.
    ///
    /// ## Returns
    /// - `Ok(Proprietor)` containing the pool's manager
    /// - `Err(DispatchError)` if the pool does not exist
    fn get_manager(
        reason: &Self::Reason,
        pool_of: &Self::Digest,
    ) -> Result<Proprietor, DispatchError>;

    /// Retrieves the commission rate associated with the specified pool.
    ///
    /// Returns the fixed commission structure that determines what portion
    /// of the pool's yield or value the manager is entitled to receive.
    ///
    /// ## Returns
    /// - `Ok(Commission)` containing the pool's commission structure
    /// - `Err(DispatchError)` if the pool does not exist
    fn get_commission(
        reason: &Self::Reason,
        pool_of: &Self::Digest,
    ) -> Result<Self::Commission, DispatchError>;

    /// Retrieves the complete pool structure for the given reason and pool digest.
    ///
    /// Returns the full pool object containing all slots, shares, manager identity,
    /// commission details, and associated metadata for inspection or processing.
    ///
    /// ## Returns
    /// - `Ok(Pool)` containing the complete pool structure
    /// - `Err(DispatchError)` if the pool does not exist
    fn get_pool(reason: &Self::Reason, pool_of: &Self::Digest)
        -> Result<Self::Pool, DispatchError>;

    /// Retrieves the real-time aggregated value of the specified pool.
    ///
    /// Computes the total value by summing the current values of all slot digests
    /// under the pool. Since the supertrait [`Commitment`] allows digest values
    /// to be updated via [`Commitment::set_digest_value`], this reflects the
    /// **live state** rather than historical deposits.
    ///
    /// The aggregated value represents the total exposure of all depositors
    /// across all slots managed by the pool.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the total aggregated value of the pool
    /// - `Err(DispatchError)` if the pool does not exist or value computation fails
    fn get_pool_value(
        reason: &Self::Reason,
        pool_of: &Self::Digest,
    ) -> Result<Self::Asset, DispatchError> {
        Self::pool_exists(reason, pool_of)?;
        let slots_values = Self::get_slots_value(reason, pool_of)?;
        let mut value: <Self as InspectAsset<Proprietor>>::Asset = Self::Asset::zero();
        for (_, val) in slots_values {
            value = value.saturating_add(val);
        }
        Ok(value)
    }

    /// Retrieves the shares of all slots within the specified pool.
    ///
    /// Returns a list of slot digests paired with their corresponding shares,
    /// representing each slot's proportional weight within the pool's
    /// managed allocation strategy.
    ///
    /// ## Returns
    /// - `Ok(Vec<(Self::Digest, Self::Shares)>)` containing slot-share pairs
    /// - `Err(DispatchError)` if the pool does not exist or retrieval fails
    fn get_slots_shares(
        reason: &Self::Reason,
        pool_of: &Self::Digest,
    ) -> Result<Vec<(Self::Digest, Self::Shares)>, DispatchError>;

    /// Retrieves the real-time values of all slots within the specified pool.
    ///
    /// Each slot's value is fetched individually and reflects any changes since
    /// the commitment was created, as the supertrait [`Commitment`] allows digest
    /// values to be updated via [`Commitment::set_digest_value`].
    ///
    /// Returns a list of slot digests paired with their current committed values,
    /// representing the live state of the pool's allocation.
    ///
    /// ## Returns
    /// - `Ok(Vec<(Self::Digest, Self::Asset)>)` containing slot-value pairs
    /// - `Err(DispatchError)` if the pool does not exist or value retrieval fails
    fn get_slots_value(
        reason: &Self::Reason,
        pool_of: &Self::Digest,
    ) -> Result<Vec<(Self::Digest, Self::Asset)>, DispatchError> {
        Self::pool_exists(reason, pool_of)?;
        let slots = Self::get_slots_shares(reason, pool_of)?;
        let mut vec = Vec::new();
        for (slot_of, _) in slots {
            let value = Self::get_slot_value(reason, pool_of, &slot_of)?;
            vec.push((slot_of, value))
        }
        Ok(vec)
    }

    /// Retrieves the real-time committed value of a specific slot within a pool.
    ///
    /// Returns the current value for the given slot digest under the specified
    /// reason and pool. Since the supertrait [`Commitment`] allows digest values
    /// to be updated via [`Commitment::set_digest_value`], this reflects the
    /// **live state** rather than the original allocation.
    ///
    /// If no funds are available in a valid slot, it is expected to return zero.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the slot's current committed value
    /// - `Err(DispatchError)` if the pool or slot does not exist
    fn get_slot_value(
        reason: &Self::Reason,
        pool_of: &Self::Digest,
        slot_of: &Self::Digest,
    ) -> Result<Self::Asset, DispatchError>;

    /// Retrieves the real-time values of a proprietor's commitments to all slots
    /// within the specified pool.
    ///
    /// Each slot's value is computed individually for the given proprietor and
    /// reflects any changes since commitment, as the supertrait [`Commitment`]
    /// allows digest values to be updated via [`Commitment::set_digest_value`].
    ///
    /// Returns a list of slot digests paired with their current committed values
    /// for the specified proprietor, showing their proportional exposure across
    /// the pool's managed allocation.
    ///
    /// ### Returns
    /// - `Ok(Vec<(Self::Digest, Self::Asset)>)` containing slot-value pairs for the proprietor
    /// - `Err(DispatchError)` if the pool does not exist or value retrieval fails
    fn get_slots_value_for(
        who: &Proprietor,
        reason: &Self::Reason,
        pool_of: &Self::Digest,
    ) -> Result<Vec<(Self::Digest, Self::Asset)>, DispatchError> {
        Self::pool_exists(reason, pool_of)?;
        let slots = Self::get_slots_shares(reason, pool_of)?;
        let mut vec = Vec::new();
        for (slot_of, _) in slots {
            let value = Self::get_slot_value_for(who, reason, pool_of, &slot_of)?;
            vec.push((slot_of, value))
        }
        Ok(vec)
    }

    /// Retrieves the real-time value of a proprietor's commitment to a specific
    /// slot within a pool.
    ///
    /// Returns the live committed value for the given proprietor and slot digest.
    /// Since the supertrait [`Commitment`] allows digest values to be updated via
    /// [`Commitment::set_digest_value`], this reflects the **current state** rather
    /// than the deposited total.
    ///
    /// ### Returns
    /// - `Ok(Asset)` containing the proprietor's current commitment to the slot
    /// - `Err(DispatchError)` if the pool or slot does not exist
    fn get_slot_value_for(
        who: &Proprietor,
        reason: &Self::Reason,
        pool_of: &Self::Digest,
        slot_of: &Self::Digest,
    ) -> Result<Self::Asset, DispatchError>;

    /// Retrieves the real-time total value of a proprietor's commitment to a pool.
    ///
    /// Aggregates the live committed values of all slot digests within the pool
    /// for the given proprietor. Since the supertrait [`Commitment`] allows digest
    /// values to be updated via [`Commitment::set_digest_value`], this reflects
    /// the **current total** rather than historical deposits.
    ///
    /// Represents the proprietor's total exposure to the pool's managed allocation
    /// strategy.
    ///
    /// ### Returns
    /// - `Ok(Asset)` containing the proprietor's total commitment to the pool
    /// - `Err(DispatchError)` if the pool does not exist or computation fails
    fn get_pool_value_for(
        who: &Proprietor,
        reason: &Self::Reason,
        pool_of: &Self::Digest,
    ) -> Result<Self::Asset, DispatchError> {
        Self::pool_exists(reason, pool_of)?;
        let mut value = Self::Asset::zero();
        let slots = Self::get_slots_value_for(who, reason, pool_of)?;
        for (_, val) in slots {
            value = value.saturating_add(val)
        }
        Ok(value)
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ````````````````````````````````` CONSTRUCTORS ````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Generates a unique digest that serves as the identifier for a pool configuration.
    ///
    /// Creates a collision-resistant identifier derived from:
    /// - The manager's identity
    /// - The reason (category/purpose)
    /// - The underlying index digest
    /// - The commission structure
    ///
    /// The generated digest acts as a **globally unique identifier** across all pools,
    /// ensuring that even similar configurations or identical indexes result in
    /// distinct pool identities when managed by different entities or with different
    /// commission rates.
    ///
    /// ## Returns
    /// - `Ok(Digest)` containing the generated unique pool digest
    /// - `Err(DispatchError)` if digest generation fails due to invalid inputs
    fn gen_pool_digest(
        manager: &Proprietor,
        reason: &Self::Reason,
        index_of: &Self::Digest,
        commission: Self::Commission,
    ) -> Result<Self::Digest, DispatchError>;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` MUTATORS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Creates a managed pool from an existing index with a fixed commission rate.
    ///
    /// Links the pool to an existing index structure under the specified reason
    /// and pool digest, establishing a managed aggregation layer with dynamic
    /// allocation capabilities.
    ///
    /// This method:
    /// - Validates that the base index exists and is accessible
    /// - Ensures the pool digest is unique and available
    /// - Records the manager (who) and immutable commission rate
    /// - Enables trustless participation for depositors who rely on the index structure
    ///
    /// **Pools can be created from any valid index**, regardless of who created it,
    /// enabling permissionless pool creation while maintaining security.
    ///
    /// Once created, the pool introduces **managed control and commission logic** over
    /// the linked index, allowing the manager to dynamically update slots, adjust slot shares
    /// and allocations.
    ///
    /// ## Returns
    /// - `Ok(())` if the pool is successfully created
    /// - `Err(DispatchError)` if the index does not exist, pool digest conflicts, or validation fails
    fn set_pool(
        who: &Proprietor,
        reason: &Self::Reason,
        pool_of: &Self::Digest,
        index_of: &Self::Digest,
        commission: Self::Commission,
    ) -> DispatchResult;

    /// Assigns or updates the manager of an existing pool.
    ///
    /// Transfers management control of the pool to a new proprietor, who then
    /// assumes responsibility for slot configuration, share adjustments, and commission control.
    ///
    /// This operation preserves all existing commitments, shares, and commission
    /// structures while changing only the entity authorized to manage the pool.
    ///
    /// ### Returns
    /// - `Ok(())` if the manager is successfully updated
    /// - `Err(DispatchError)` if the pool does not exist or authorization fails
    fn set_pool_manager(
        reason: &Self::Reason,
        pool_of: &Self::Digest,
        manager: &Proprietor,
    ) -> DispatchResult;

    /// Updates the shares of a specific slot within an existing pool.
    ///
    /// Allows the pool manager to dynamically reallocate exposure between slots
    /// without recreating the entire pool structure. This enables responsive
    /// strategy adjustments based on market conditions, performance metrics,
    /// or risk management requirements.
    ///
    /// The operation preserves all existing commitments while adjusting the
    /// proportional weight of the specified slot within the pool's allocation.
    ///
    /// ### Returns
    /// - `Ok(())` if the slot shares are successfully updated
    /// - `Err(DispatchError)` if the pool or slot does not exist, or authorization fails
    fn set_slot_shares(
        who: &Proprietor,
        reason: &Self::Reason,
        pool_of: &Self::Digest,
        slot_of: &Self::Digest,
        shares: Self::Shares,
    ) -> DispatchResult;

    /// Creates a new pool digest from an index with a specified commission rate.
    ///
    /// Generates a unique pool identifier by combining the index structure with
    /// a commission rate, enabling multiple pools with different commission
    /// structures to be created from the same underlying index.
    ///
    /// Since commission rates are **immutable** after pool creation to protect
    /// depositors, this method allows participants to create or select pools
    /// with their preferred commission terms without altering existing pools.
    ///
    /// The generated digest serves as the unique identifier for the new pool
    /// configuration and can be used for subsequent pool operations.
    ///
    /// ## Returns
    /// - `Ok(Digest)` containing the newly generated pool digest
    /// - `Err(DispatchError)` if pool creation fails due to invalid parameters or conflicts
    fn set_commission(
        who: &Proprietor,
        reason: &Self::Reason,
        index_of: &Self::Digest,
        commission: Self::Commission,
    ) -> Result<Self::Digest, DispatchError> {
        let pool_of = Self::gen_pool_digest(who, reason, index_of, commission)?;
        Self::set_pool(who, reason, &pool_of, index_of, commission)?;
        Ok(pool_of)
    }

    /// Removes a pool if it contains no active commitments.
    ///
    /// Permanently deletes the specified pool digest under the given reason,
    /// freeing associated resources and references. Pools can only be reaped when
    /// **no proprietors have active commitments** to any of its slots.
    ///
    /// Attempting to reap a non-empty pool must return an error.
    ///
    /// ## Returns
    /// - `Ok(())` if the pool is successfully removed
    /// - `Err(DispatchError)` if the pool does not exist or contains active commitments
    fn reap_pool(reason: &Self::Reason, pool_of: &Self::Digest) -> DispatchResult;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````````` HOOKS ````````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Hook called after a pool is successfully created by a proprietor (assumed to be
    /// the current pool-manager).
    ///
    /// Provides an extension point for triggering side-effects such as events,
    /// logging, recalculations, or external state updates when a pool is
    /// established or modified.
    ///
    /// Default implementation is a no-op.
    fn on_set_pool(
        _who: &Proprietor,
        _pool_of: &Self::Digest,
        _reason: &Self::Reason,
        _pool: &Self::Pool,
    ) {
    }

    /// Hook called after slot shares are updated within a pool.
    ///
    /// Provides an extension point for triggering recalculations, propagating
    /// changes to related state, or emitting events when a slot's shares are
    /// adjusted as part of the pool's dynamic allocation strategy.
    ///
    /// Default implementation is a no-op.
    fn on_set_slot_shares(
        _pool_of: &Self::Digest,
        _reason: &Self::Reason,
        _slot_of: &Self::Digest,
        _shares: Self::Shares,
    ) {
    }

    /// Hook called after a pool's manager is changed.
    ///
    /// Provides an extension point for triggering governance events, audit logging,
    /// or external notifications when management control of a pool is transferred
    /// to a new proprietor.
    ///
    /// Default implementation is a no-op.
    fn on_set_manager(_pool_of: &Self::Digest, _reason: &Self::Reason, _manager: &Proprietor) {}

    /// Hook called after a pool is reaped (removed).
    ///
    /// Provides an extension point for cleanup tasks, audit logging, freeing
    /// related resources, or triggering external notifications when a pool
    /// is permanently removed from the system.
    ///
    /// `dust` represents any residual value that was unclaimable and
    /// effectively considered dead.
    ///
    /// Default implementation is a no-op.
    fn on_reap_pool(_pool_of: &Self::Digest, _reason: &Self::Reason, _dust: Self::Asset) {}
}

// ===============================================================================
// ``````````````````````````````` COMMIT VARIANT ````````````````````````````````
// ===============================================================================

/// `CommitVariant` extends [`Commitment`] by introducing **variant-based commitments** -
/// allowing each commitment to represent a specific *position* or *type*,
/// such as positive/negative, long/short, or other directional states.
///
/// This abstraction is useful in financial systems where commitments
/// can have opposing or complementary meanings (e.g., bets, trades, or hedges),
/// and where such **variants** must be consistently classified and tracked
/// under the same digest or reason.
///
/// ## Purpose
///
/// In standard [`Commitment`], each digest represents a single locked or committed value.
/// However, many financial instruments carry a **semantic variant**, such as:
/// - Positive / Negative exposure
/// - Long / Short position
/// - Buy / Sell order
/// - For / Against vote
///
/// `CommitVariant` provides an abstract way to encode and manage such distinctions
/// without altering the base commitment model.
///
/// ## Core Principles
///
/// 1. **Variant as classification**
///    - Each commitment may declare a variant indicating its nature or direction.
///    - Variants are accounted for directly under the commitment's digest.
///
/// 2. **Digest-level association**
///    - A commitment's digest uniquely identifies both its value and variant.
///    - Variants must be recorded or derivable from the digest state.
///
/// 3. **Financial semantics**
///    - Designed for use in scenarios like betting, derivatives, prediction markets,
///      and financial trilemmas where commitments can oppose or complement each other.
///
/// ## Example Use Cases
///
/// - **Betting systems**: Represent "for" and "against" commitments.
/// - **Trading platforms**: Track "long" and "short" positions.
/// - **Hedging mechanisms**: Manage opposing commitments under one reason.
/// - **Voting protocols**: Represent affirmative and negative stake commitments.
///
/// ## Summary
///
/// `CommitVariant` offers an abstract and extensible layer over [`Commitment`],
/// enabling richer semantics and directionality (positive/negative or other variants)
/// to be embedded into financial commitment logic in a consistent, trustless manner.
///
/// Generics:
/// - **Proprietor** - the entity that owns the asset and can make commitments.
pub trait CommitVariant<Proprietor>: Commitment<Proprietor> {
    /// The type representing a commitment's position or variant.
    type Position: RuntimeEnum + Delimited;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` CHECKERS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Validates whether a specific digest variant's value can be set or updated.
    ///
    /// Ensures that:
    /// - The variant is initialized (has a balance entry)
    /// - The intended update respects mint/reap advisory limits
    ///
    /// ## Returns
    /// - `Ok(())` if validation succeeds
    /// - `Err(DispatchError)` if the digest/variant is invalid or limits are violated
    fn can_set_digest_variant_value(
        reason: &Self::Reason,
        digest: &Self::Digest,
        value: Self::Asset,
        variant: &Self::Position,
        qualifier: &Self::Intent,
    ) -> DispatchResult {
        let current = Self::get_digest_variant_value(reason, digest, variant)?;
        match current.cmp(&value) {
            Ordering::Less => {
                // Mint path (increase)
                let limits = Self::digest_mint_limits(digest, reason, qualifier)?;
                let mintable = value.saturating_sub(current);
                ensure!(
                    limits.contains(mintable),
                    Self::from_commit_error(CommitError::MintingOffLimits)
                )
            }
            Ordering::Greater => {
                // Reap path (decrease)
                let limits = Self::digest_reap_limits(digest, reason, qualifier)?;
                let reapable = current.saturating_sub(value);
                ensure!(
                    limits.contains(reapable),
                    Self::from_commit_error(CommitError::ReapingOffLimits)
                )
            }
            Ordering::Equal => {
                // No-op
            }
        }

        Ok(())
    }

    /// Validates whether a new commitment can be placed for a specific variant.
    ///
    /// Ensures that no existing commitment exists for the given reason (enforcing
    /// the "one digest per reason" invariant) and that the proprietor has sufficient
    /// available funds to cover the commitment value.
    ///
    /// Additionally validates that the value satisfies variant-specific placement
    /// limits derived from the lazy balance model.
    ///
    /// ## Returns
    /// - `Ok(())` if validation succeeds
    /// - `Err(DispatchError)` if a commitment already exists, insufficient funds are available,
    ///   or the value violates variant-specific limits
    fn can_place_commit_of_variant(
        who: &Proprietor,
        reason: &Self::Reason,
        digest: &Self::Digest,
        variant: &Self::Position,
        value: Self::Asset,
        qualifier: &Self::Intent,
    ) -> DispatchResult {
        ensure!(
            Self::commit_exists(who, reason).is_err(),
            Self::from_commit_error(CommitError::CommitAlreadyExists)
        );
        let max = Self::available_funds(who);
        ensure!(
            max >= value,
            Self::from_commit_error(CommitError::InsufficientFunds)
        );
        let limits = Self::place_commit_limits_of_variant(who, reason, digest, variant, qualifier)?;
        ensure!(
            limits.contains(value),
            Self::from_commit_error(CommitError::PlacingOffLimits)
        );

        Ok(())
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` GETTERS ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Retrieves the position/variant of a proprietor's commitment for
    /// the given reason.
    ///
    /// Returns the active variant under which the proprietor's commitment
    /// is currently classified. Since each proprietor can have only **one
    /// commitment per reason** (base trait invariant), they also have only
    /// **one active variant per reason**.
    ///
    /// ## Returns
    /// - `Ok(Position)` containing the commitment's current variant
    /// - `Err(DispatchError)` if no commitment exists for the reason
    fn get_commit_variant(
        who: &Proprietor,
        reason: &Self::Reason,
    ) -> Result<Self::Position, DispatchError>;

    /// Retrieves the aggregated value for a specific digest and variant combination.
    ///
    /// Returns the real-time sum of all commitments to the given digest that are
    /// classified under the specified variant. This enables querying variant-specific
    /// exposure across all proprietors.
    ///
    /// When both [`Commitment`] and `CommitVariant` are implemented:
    /// - [`Commitment::get_digest_value`] returns the aggregate across **all variants**
    /// - This method returns the aggregate for **a specific variant**
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the variant-specific aggregated value
    /// - `Err(DispatchError)` if the digest or variant does not exist
    fn get_digest_variant_value(
        reason: &Self::Reason,
        digest: &Self::Digest,
        variant: &Self::Position,
    ) -> Result<Self::Asset, DispatchError>;

    /// Provides advisory bounds for increasing (minting) a digest's
    /// specific variant-balance.
    ///
    /// Acts as an optional guard to prevent abrupt or excessive growth,
    /// complementing core digest update logic.
    fn digest_mint_limits_of_variant(
        _digest: &Self::Digest,
        _reason: &Self::Reason,
        _variant: &Self::Position,
        _qualifier: &Self::Intent,
    ) -> Result<Self::Limits, DispatchError> {
        // By default, this returns a baseline limit,
        // representing a static or fallback bound when no dynamic limits are applied.
        //
        // Implementors may override this to provide context-specific limits.
        Ok(Self::Limits::none())
    }

    /// Provides advisory bounds for decreasing (reaping) a digest's
    /// specific variant-balance.
    ///
    /// Acts as an optional guard to prevent aggressive or premature reductions,
    /// complementing core invariants that ensure correctness.
    fn digest_reap_limits_of_variant(
        _digest: &Self::Digest,
        _reason: &Self::Reason,
        _variant: &Self::Position,
        _qualifier: &Self::Intent,
    ) -> Result<Self::Limits, DispatchError> {
        // By default, this returns a baseline limit,
        // representing a static or fallback bound when no dynamic limits are applied.
        // Implementors may override this to provide context-specific limits.
        Ok(Self::Limits::none())
    }

    /// Provides advisory bounds for placing a new commitment
    /// under a given variant.
    ///
    /// Acts as an optional pre-validation layer to prevent premature or
    /// undesirable commits by constraining value ranges.
    fn place_commit_limits_of_variant(
        _who: &Proprietor,
        _reason: &Self::Reason,
        _digest: &Self::Digest,
        _variant: &Self::Position,
        _qualifier: &Self::Intent,
    ) -> Result<Self::Limits, DispatchError> {
        // By default, this returns an unbounded extent,
        // meaning no additional constraints are applied unless overridden.
        //
        // Implementors may override this to provide context-specific limits.
        Ok(Self::Limits::none())
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` MUTATORS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Sets or updates the aggregated value for a specific digest and variant.
    ///
    /// Updates the value associated with a particular variant of a digest,
    /// propagating changes to all commitments tied to that digest-variant pair.
    ///
    /// When both [`Commitment`] and [`CommitVariant`] are utilized in a system:
    /// - [`Commitment::set_digest_value`] may set the **default variant**.
    /// - This method updates **the specified variant**.
    ///
    /// The `qualifier` defines how the update is applied (e.g., exact vs best-effort,
    /// forceful vs bounded), and may influence the final value that is actually set.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the actual value applied to the specified digest-variant
    /// - `Err(DispatchError)` if the digest, variant, or reason is invalid, or update fails
    fn set_digest_variant_value(
        reason: &Self::Reason,
        digest: &Self::Digest,
        value: Self::Asset,
        variant: &Self::Position,
        qualifier: &Self::Intent,
    ) -> Result<Self::Asset, DispatchError>;

    /// Changes the variant of an existing commitment.
    ///
    /// Transitions a commitment from its current variant to a new variant while
    /// preserving the committed value. Since commitments are immutable, this
    /// operation uses the **resolve-and-replace pattern**.
    ///
    /// Ensures that variant changes maintain consistency with the underlying
    /// commitment state and respect the semantics of the commitment system.
    ///
    /// In case if the variant already exists, the function safely returns.
    ///
    /// ## Returns
    /// - `Ok(())` if the variant is successfully changed
    /// - `Err(DispatchError)` if:
    ///   - No commitment exists for the reason
    ///   - New variant matches current variant
    ///   - Commitment resolution or placement fails
    fn set_commit_variant(
        who: &Proprietor,
        reason: &Self::Reason,
        variant: &Self::Position,
        qualifier: &Self::Intent,
    ) -> DispatchResult {
        Self::commit_exists(who, reason)?;
        let current_variant = Self::get_commit_variant(who, reason)?;
        if current_variant == *variant {
            return Ok(());
        }
        let digest = Self::get_commit_digest(who, reason)?;
        let re_deposit = Self::resolve_commit(who, reason)?;
        Self::place_commit_of_variant(who, reason, &digest, re_deposit, variant, qualifier)?;
        Ok(())
    }

    /// Places a commitment under a specific variant.
    ///
    /// Creates a new commitment with explicit variant classification, enabling
    /// directional or positional semantics from the moment of placement.
    ///
    /// This extends [`Commitment::place_commit`] by adding variant specification.
    /// When both traits are implemented:
    /// - [`Commitment::place_commit`] may use a **default variant**
    /// - This method allows **explicit variant selection**
    ///
    /// The method must:
    /// - Record the commitment under the specified variant
    /// - Associate the variant with the digest at the commitment level
    /// - Respect qualifier parameters
    /// - Return the actual committed value after any adjustments
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the actual value committed under the variant
    /// - `Err(DispatchError)` if placement fails.
    fn place_commit_of_variant(
        who: &Proprietor,
        reason: &Self::Reason,
        digest: &Self::Digest,
        value: Self::Asset,
        variant: &Self::Position,
        qualifier: &Self::Intent,
    ) -> Result<Self::Asset, DispatchError>;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````````` HOOKS ````````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Hook called after placing a commitment under a specific variant.
    ///
    /// Provides an extension point for triggering custom side-effects when a new
    /// variant-based commitment is placed.
    ///
    /// Default implementation is a no-op.
    fn on_place_commit_on_variant(
        _who: &Proprietor,
        _reason: &Self::Reason,
        _digest: &Self::Digest,
        _value: Self::Asset,
        _variant: &Self::Position,
    ) {
    }

    /// Hook called after a commitment's variant is changed.
    ///
    /// Provides an extension point for responding to variant transitions.
    /// Receives both the digest and value to enable comprehensive
    /// state management and analytics.
    ///
    /// Default implementation is a no-op.
    fn on_set_commit_variant(
        _who: &Proprietor,
        _reason: &Self::Reason,
        _digest: &Self::Digest,
        _value: Self::Asset,
        _variant: &Self::Position,
    ) {
    }

    /// Hook called after a digest's variant value is updated.
    ///
    /// Provides an extension point for reacting to variant-specific value changes.
    ///
    /// Default implementation is a no-op.
    fn on_set_digest_variant(
        _digest: &Self::Digest,
        _reason: &Self::Reason,
        _value: Self::Asset,
        _variant: &Self::Position,
    ) {
    }
}

// ===============================================================================
// ``````````````````````````````` INDEX VARIANT `````````````````````````````````
// ===============================================================================

/// A trait for managing and querying **variants** of index entries and its commitments.
///
/// Extends [`CommitVariant`] and [`CommitIndex`], allowing indexed commitments
/// to carry additional positional/variant metadata.  
///
/// This trait enables tracking, setting, and preparing variant entries in an index context.
///
/// Generics:
/// - **Proprietor** - the entity that owns the asset and can make commitments.
pub trait IndexVariant<Proprietor>: CommitVariant<Proprietor> + CommitIndex<Proprietor> {
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ````````````````````````````````` CONSTRUCTORS ````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Prepares an index from entry data with explicit variant classification.
    ///
    /// Constructs a variant-aware index structure from a list of `(Digest, Shares, Position)` tuples,
    /// where each tuple represents:
    /// - **Digest**: The unique identifier of an entry within the index
    /// - **Shares**: The proportional weight or ownership of that entry
    /// - **Position**: The variant classification for that entry
    ///
    /// This extends [`CommitIndex::prepare_index`] by adding variant specification for each entry.
    /// When both traits are implemented:
    /// - [`CommitIndex::prepare_index`] may assign a **default variant** to all entries
    /// - This method allows **explicit variant specification** per entry
    ///
    /// ## Returns
    /// - `Ok(Index)` containing the prepared variant-aware index structure
    /// - `Err(DispatchError)` if preparation fails
    fn prepare_index_of_variants(
        who: &Proprietor,
        reason: &Self::Reason,
        entries: Vec<(Self::Digest, Self::Shares, Self::Position)>,
    ) -> Result<Self::Index, DispatchError>;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` GETTERS ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Retrieves the variant associated with a specific entry within an index.
    ///
    /// Returns the position classification for the given entry digest under the
    /// specified reason and index digest. This enables querying the directional
    /// or positional semantics of individual entries within a mixed-variant index.
    ///
    /// ## Returns
    /// - `Ok(Position)` containing the entry's variant
    /// - `Err(DispatchError)` if the index or entry does not exist
    fn get_entry_variant(
        reason: &Self::Reason,
        index_of: &Self::Digest,
        entry_of: &Self::Digest,
    ) -> Result<Self::Position, DispatchError>;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` MUTATORS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Associates or updates a variant for a specific entry within an index.
    ///
    /// Records a position (variant) for an entry digest within the context of a given
    /// index digest and reason. If shares are provided, they are updated alongside the variant.
    ///
    /// Since indexes are **immutable** once created, this method internally
    /// generates a new digest for the modified index and returns the new index digest.
    ///
    /// For updating multiple entry variants simultaneously, use
    /// [`IndexVariant::prepare_index_of_variants`] to construct a completely new index structure.
    ///
    /// This behavior partly mirrors [`CommitIndex::set_index`], and can be
    /// considered a higher-level wrapper around index reconstruction and binding.
    ///
    /// As such, it is assumed that any side-effects or lifecycle handling are
    /// delegated through the underlying [`CommitIndex::set_index`] call, and
    /// therefore this method does **not** introduce a separate hook.
    ///
    /// ## Returns
    /// - `Ok(Digest)` containing the new index digest after variant update
    /// - `Err(DispatchError)` if the index or entry does not exist, or update fails
    fn set_entry_of_variant(
        who: &Proprietor,
        reason: &Self::Reason,
        index_of: &Self::Digest,
        entry_of: &Self::Digest,
        variant: Self::Position,
        shares: Option<Self::Shares>,
    ) -> Result<Self::Digest, DispatchError>;
}

// ===============================================================================
// ```````````````````````````````` POOL VARIANT `````````````````````````````````
// ===============================================================================

/// A trait for managing and querying **variants** of pool slots and their commitments.
///
/// Extends [`CommitPool`] to allow pool commitments to carry additional positional
/// or variant metadata.
///
/// Provides the ability to retrieve, set, and react to variant changes within a pool context.
///
/// Generics:
/// - **Proprietor** - the entity that owns the asset and can make commitments.
pub trait PoolVariant<Proprietor>:
    CommitVariant<Proprietor> + CommitPool<Proprietor> + CommitIndex<Proprietor>
{
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` GETTERS ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Retrieves the variant (position) associated with a specific slot within a pool.
    ///
    /// Returns the position classification for the given slot digest under the
    /// specified reason and pool digest. This enables querying the directional
    /// or positional semantics of individual slots within a managed variant-aware pool.
    ///
    /// ## Returns
    /// - `Ok(Position)` containing the slot's variant
    /// - `Err(DispatchError)` if the pool or slot does not exist
    fn get_slot_variant(
        reason: &Self::Reason,
        pool_of: &Self::Digest,
        slot_of: &Self::Digest,
    ) -> Result<Self::Position, DispatchError>;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` MUTATORS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Associates or updates a variant for a specific slot within a pool.
    ///
    /// Records a position (variant) for a slot digest within the context of a given
    /// pool digest and reason. If shares are provided, they are updated alongside the variant.
    ///
    /// Since pools are **mutable** (unlike indexes), this method directly updates
    /// the slot's variant without creating a new pool digest.
    ///
    /// This is a **managed operation** - typically restricted to the pool manager
    /// or authorized entities, ensuring controlled strategy execution while
    /// protecting depositor interests.
    ///
    /// ## Returns
    /// - `Ok(())` if the variant is successfully updated
    /// - `Err(DispatchError)` if fails
    fn set_slot_of_variant(
        who: &Proprietor,
        reason: &Self::Reason,
        pool_of: &Self::Digest,
        slot_of: &Self::Digest,
        variant: Self::Position,
        shares: Option<Self::Shares>,
    ) -> DispatchResult;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````````` HOOKS ````````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Hook called after a slot variant is updated within a pool.
    ///
    /// Provides an extension point for triggering side-effects when a slot's
    /// variant is associated or changed within a managed pool.
    ///
    /// Receives both the slot digest and optional shares to enable comprehensive
    /// state management and analytics when managed slot configurations change.
    ///
    /// Default implementation is a no-op.
    fn on_set_slot_of_variant(
        _pool_of: &Self::Digest,
        _reason: &Self::Reason,
        _slot_of: &Self::Digest,
        _shares: Option<Self::Shares>,
        _variant: &Self::Position,
    ) {
    }
}
