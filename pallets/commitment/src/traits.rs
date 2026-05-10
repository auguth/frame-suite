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
// ```````````````````````````` COMMIT-HELPERS TRAITS ````````````````````````````
// ===============================================================================

//! Defines helper traits for implementing a concrete [`Commitment`] system.
//! All operations are **low-level and unchecked** - callers must ensure validity,
//! equilibrium, and invariants before invoking these functions.
//!
//! ## Requirements
//! This module additionally requires the balance primitive [`LazyBalance`] provided
//! via the implementing pallet. All balance interactions and accounting are expected
//! to be handled through this abstraction.
//!
//! ## Traits
//! - [`CommitBalance`] - balance management and reconciliation.
//! - [`CommitDeposit`] - low-level deposit operations.
//! - [`CommitWithdraw`] - low-level withdrawal operations.
//! - [`CommitOps`] - placing, raising, and resolving commitments.
//! - [`CommitInspect`] - inspection and auditing of committed values.
//! - [`PoolOps`] - operations for pooled commitments.
//! - [`IndexOps`] - operations for indexed digest commitments.
//!
//! ## Design Principles
//! - Explicit imbalance handling for auditability and safety.
//! - Composability to build full commitment systems (direct, index, pool).
//! - Separation of low-level operations from higher-level safety.
//! - Equillibrium in Queriable values maintained.

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- FRAME Suite ---
use frame_suite::{assets::LazyBalance, commitment::*};

// --- Substrate primitives ---
use sp_runtime::{
    traits::{CheckedAdd, CheckedSub},
    ArithmeticError, DispatchError, DispatchResult,
};

// ===============================================================================
// ```````````````````````````````` COMMIT BALANCE ```````````````````````````````
// ===============================================================================

/// Provides low-level balance management and reconciliation behavior
/// for [`Commitment`] systems.
///
/// Commitment frameworks often employ *unbalanced fungible traits* rather than
/// automatically balanced ones. This enables explicit safety enforcement and
/// auditability by requiring all balance mismatches to be resolved intentionally
/// rather than implicitly.  
///
/// This trait defines how those imbalances and balance adjustments should be
/// handled consistently across the system.
///
/// It only defines **low-level, unchecked operations** - callers are
/// responsible for ensuring validity and equilibrium before invoking this
/// function.
///
/// ### Generics
/// - **Proprietor** - the entity (e.g. account, vault, or manager) owning
///   or controlling the underlying asset balance.
/// - **Pallet** - the pallet public struct which implements [`Commitment`] traits
///   for consumer pallet usage, and provides the required commitment abstraction.
/// - The implementing `Pallet` **must provide [`LazyBalance`]** with guarantees
///   that all balance operations remain consistent with the commitment system's
///   underlying asset accounting.
pub trait CommitBalance<Proprietor, Pallet>
where
    Pallet: LazyBalance<Asset = <Pallet as InspectAsset<Proprietor>>::Asset>
        + InspectAsset<Proprietor>
        + Commitment<Proprietor>,
{
    /// The imbalance type representing the difference between deposits and withdrawals.
    type Imbalance;

    /// Resolves an imbalance in a proprietor's committed balance.
    ///
    /// Used when a digest's value adjustment causes a mismatch in
    /// deposited versus withdrawn amounts.  
    ///
    /// Ensures the underlying asset accounting remains correct by minting,
    /// burning, or otherwise reconciling the imbalance.
    ///
    /// It only defines **low-level, unchecked operations** - callers are
    /// responsible for ensuring validity and equilibrium before invoking this
    /// function.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the final balanced value
    /// - `Err(DispatchError)` if reconciliation fails
    fn resolve_imbalance(
        who: &Proprietor,
        imbalance: Self::Imbalance,
    ) -> Result<<Pallet as InspectAsset<Proprietor>>::Asset, DispatchError>;

    /// Deducts a specified asset value from the proprietor's available balance.
    ///
    /// This function deducts the requested amount according to the given
    /// qualifier - exactness or best effort along with force rules to
    /// determine whether and how the deduction should proceed.
    ///
    /// It only defines **low-level, unchecked operations** - callers are
    /// responsible for ensuring validity and equilibrium before invoking this
    /// function.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the actual deducted value
    /// - `Err(DispatchError)` if the deduction fails
    fn deduct_balance(
        who: &Proprietor,
        value: <Pallet as InspectAsset<Proprietor>>::Asset,
        qualifier: &Pallet::Intent,
    ) -> Result<<Pallet as InspectAsset<Proprietor>>::Asset, DispatchError>;

    /// Deducts a specified asset value from a imbalance (effectively mutating it).
    ///
    /// This function deducts the exact amount, maintains equillibrium and
    /// returns another imbalance - to be resolved (typically by [`Self::resolve_imbalance`]).
    ///
    /// This provides extracting new-imbalances from existing imbalances safely.
    ///
    /// It only defines **low-level, unchecked operations** - callers are
    /// responsible for ensuring validity and equilibrium before invoking this
    /// function.
    ///
    /// ## Returns
    /// - `Ok(Imbalance)` containing the new imbalance
    /// - `Err(DispatchError)` if the deduction fails
    fn deduct_from_imbalance(
        imbalance: &mut Self::Imbalance,
        value: <Pallet as InspectAsset<Proprietor>>::Asset,
    ) -> Result<Self::Imbalance, DispatchError>;
}

// ===============================================================================
// ```````````````````````````````` COMMIT DEPOSIT ```````````````````````````````
// ===============================================================================

/// Provides low-level deposit operations for commitment systems, enabling assets
/// to be recorded against digests, indexes, and pools for a given proprietor.
///
/// This trait focuses solely on **recording deposits** within the system. It does **not**
/// handle balance deductions, freezing funds, or higher-level commit management - those
/// responsibilities are delegated to upper layers of the [`Commitment`] framework.
///
/// It only defines **low-level, unchecked operations** - callers are
/// responsible for ensuring validity and equilibrium before invoking these
/// functions.
///
/// ### Generics
/// - **Proprietor** - the entity (e.g. account, vault, or manager)
/// controlling the asset.
/// - **Pallet** - the public struct implementing [`Commitment`] traits
/// and [`LazyBalance`], ensuring consistent asset accounting across the
/// commitment system.
pub trait CommitDeposit<Proprietor, Pallet>
where
    Pallet: LazyBalance<
            Asset = <Pallet as InspectAsset<Proprietor>>::Asset,
            Variant = Pallet::Position,
            Id = Pallet::Digest,
        > + DigestModel<Proprietor>
        + CommitVariant<Proprietor>,
{
    /// Type representing a "Receipt" of the deposit towards the
    /// digest (direct) at the time of commitment.
    ///
    /// A receipt acts as a claimable bill over the digest's balance,
    /// capturing the state at the time of commitment. This allows
    /// commitments to be resolved accurately later, even if the
    /// underlying balance changes.
    ///
    /// Effectively serves as a versioned claim tied to a specific
    /// commitment.
    type Receipt;

    /// Deposits a value to a fully resolved/wrapped digest model and it's
    /// specified balance variant.
    ///
    /// This function assumes the digest has already been resolved and wrapped
    /// appropriately, so it directly handles the act of **only depositing**
    /// the given value under the specified reason.
    ///
    /// This is a **low-level** function that only records the deposit to the
    /// digest model. It does **not** perform balance deductions or handle
    /// commit-level operations such as freezing funds or storing commitment
    /// records. Any such logic must be handled at a higher level.
    ///
    /// Some digests such as indexes and pools may not require a `variant`, yet
    /// direct requires it, so its kept reagrdless and the caller must acknowledge
    /// that.
    ///
    /// ## Returns
    /// - `Ok((Receipt, Asset))` containing the deposit's state and the
    /// actual deposit value.
    /// - `Err(DispatchError)` if the deposit fails
    fn deposit_to(
        who: &Proprietor,
        reason: &Pallet::Reason,
        digest_model: &Pallet::Model,
        value: <Pallet as InspectAsset<Proprietor>>::Asset,
        variant: &Pallet::Position,
        qualifier: &Pallet::Intent,
    ) -> Result<(Self::Receipt, <Pallet as InspectAsset<Proprietor>>::Asset), DispatchError>;

    /// Deposits a committed asset into a given digest of variant for
    /// a specified reason.
    ///
    /// This is a **low-level** function that only records the deposit to
    /// the digest. It does **not** perform balance deductions or handle
    /// commit-level operations such as freezing funds or storing commitment
    /// records. Any such logic must be handled at a higher level.
    ///
    /// ## Returns
    /// - `Ok((Receipt, Asset))` containing the deposit's state and the
    /// actual deposit value.
    /// - `Err(DispatchError)` if the deposit fails
    fn deposit_to_digest(
        who: &Proprietor,
        reason: &Pallet::Reason,
        digest: &Pallet::Digest,
        value: <Pallet as InspectAsset<Proprietor>>::Asset,
        variant: &Pallet::Position,
        qualifier: &Pallet::Intent,
    ) -> Result<(Self::Receipt, <Pallet as InspectAsset<Proprietor>>::Asset), DispatchError>;

    /// Deposits a committed asset into a given index for a specified reason.
    ///
    /// This is a **low-level** function that only records the deposit to the index.
    /// It does **not** perform balance deductions or handle commit-level operations
    /// such as freezing funds or storing commitment records.  
    /// Any such logic must be handled at a higher level.
    ///
    /// ## Returns
    /// - `Ok((Receipt, Asset))` containing the deposit's state and the
    /// actual deposit value.
    /// - `Err(DispatchError)` if the deposit fails
    fn deposit_to_index(
        who: &Proprietor,
        reason: &Pallet::Reason,
        index_of: &Pallet::Digest,
        value: <Pallet as InspectAsset<Proprietor>>::Asset,
        variant: &Pallet::Position,
        qualifier: &Pallet::Intent,
    ) -> Result<(Self::Receipt, <Pallet as InspectAsset<Proprietor>>::Asset), DispatchError>;

    /// Deposits a committed asset into a given pool of a variant for a specified reason.
    ///
    /// This is a **low-level** function that only records the deposit to the pool.
    /// It does **not** perform balance deductions or handle commit-level operations
    /// such as freezing funds or storing commitment records.  
    /// Any such logic must be handled at a higher level.
    ///
    /// ## Returns
    /// - `Ok((Receipt, Asset))` containing the deposit's state and the
    /// actual deposit value.
    /// - `Err(DispatchError)` if the deposit fails
    fn deposit_to_pool(
        who: &Proprietor,
        reason: &Pallet::Reason,
        pool_of: &Pallet::Digest,
        value: <Pallet as InspectAsset<Proprietor>>::Asset,
        variant: &Pallet::Position,
        qualifier: &Pallet::Intent,
    ) -> Result<(Self::Receipt, <Pallet as InspectAsset<Proprietor>>::Asset), DispatchError>;
}

// ===============================================================================
// ```````````````````````````````` COMMIT WITHDRAW ``````````````````````````````
// ===============================================================================

/// Provides low-level withdrawal operations for commitment systems, enabling proprietors
/// to extract committed assets from digests, indexes, and pools for a given reason.
///
/// This trait focuses solely on **withdrawing committed values**. It does **not**
/// handle higher-level operations such as unfreezing funds, updating commitment records,
/// or enforcing variant rules - those responsibilities are delegated to upper layers
/// of the [`Commitment`] framework.
///
/// It only defines **low-level, unchecked operations** - callers are
/// responsible for ensuring validity and equilibrium before invoking these
/// functions.
///
/// ### Generics
/// - **Proprietor** - the entity (e.g. account, vault, or manager)
/// controlling the asset.
/// - **Pallet** - the public struct implementing [`Commitment`] traits
/// and [`LazyBalance`], ensuring consistent asset accounting across the
/// commitment system.
pub trait CommitWithdraw<Proprietor, Pallet>: CommitBalance<Proprietor, Pallet>
where
    Pallet: LazyBalance<
            Asset = <Pallet as InspectAsset<Proprietor>>::Asset,
            Variant = Pallet::Position,
            Id = Pallet::Digest,
        > + DigestModel<Proprietor>
        + CommitVariant<Proprietor>,
{
    /// Withdraws the proprietor's value of a commitment done to a resolved/wrapped
    /// digest model for a given reason.
    ///
    /// This function assumes the digest has already been resolved and wrapped appropriately,
    /// so it directly handles the act of **only withdrawing** its total committed value
    /// under the specified reason.
    ///
    /// This is a **low-level** function that only withdraws to the given proprietor.
    /// It does **not** perform commit-level operations such as unfreezing funds or
    /// updating commitment records, nor variant validation.  
    /// Any such logic must be handled at a higher level.
    ///
    /// ## Returns
    /// - `Ok(Imbalance)` containing the withdrawal imbalance
    /// - `Err(DispatchError)` if the withdrawal fails
    fn withdraw_for(
        who: &Proprietor,
        reason: &Pallet::Reason,
        digest_model: &Pallet::Model,
        variant: &Pallet::Position,
    ) -> Result<Self::Imbalance, DispatchError>;

    /// Withdraws the proprietor's value of a commitment to the given digest
    /// of variant for a specified reason.
    ///
    /// This is a **low-level** function that only withdraws to the given proprietor.
    /// It does **not** perform commit-level operations such as unfreezing funds or
    /// updating commitment records.  
    /// Any such logic must be handled at a higher level.
    ///
    /// ## Returns
    /// - `Ok(Imbalance)` containing the withdrawal imbalance
    /// - `Err(DispatchError)` if the withdrawal fails
    fn withdraw_from_digest(
        who: &Proprietor,
        reason: &Pallet::Reason,
        digest: &Pallet::Digest,
        variant: &Pallet::Position,
    ) -> Result<Self::Imbalance, DispatchError>;

    /// Withdraws the proprietor's value of a commitment to the given index
    /// digest of variant for a specified reason.
    ///
    /// This is a **low-level** function that only withdraws to the given proprietor.
    /// It does **not** perform commit-level operations such as unfreezing funds or
    /// updating commitment records.  
    /// Any such logic must be handled at a higher level.
    ///
    /// ## Returns
    /// - `Ok(Imbalance)` containing the withdrawal imbalance
    /// - `Err(DispatchError)` if the withdrawal fails
    fn withdraw_from_index(
        who: &Proprietor,
        reason: &Pallet::Reason,
        index_of: &Pallet::Digest,
        variant: &Pallet::Position,
    ) -> Result<Self::Imbalance, DispatchError>;

    /// Withdraws the proprietor's value of a commitment to the given pool digest of variant
    /// for a specified reason.
    ///
    /// This is a **low-level** function that only withdraws to the given proprietor.
    /// It does **not** perform commit-level operations such as unfreezing funds or
    /// updating commitment records.  
    /// Any such logic must be handled at a higher level.
    ///
    /// ## Returns
    /// - `Ok(Imbalance)` containing the withdrawal imbalance
    /// - `Err(DispatchError)` if the withdrawal fails
    fn withdraw_from_pool(
        who: &Proprietor,
        reason: &Pallet::Reason,
        pool_of: &Pallet::Digest,
        variant: &Pallet::Position,
    ) -> Result<Self::Imbalance, DispatchError>;
}

// ===============================================================================
// ``````````````````````````````` COMMIT OPERATIONS `````````````````````````````
// ===============================================================================

/// Defines the core operations for managing commitments within digest models.
///
/// This trait provides a unified interface for placing, raising, and resolving
/// commitments across all digest variants - direct, index, and pool.
///
/// This trait defines **low-level, unchecked operations** - callers are
/// responsible for ensuring validity and equilibrium before invoking these
/// functions.
///
/// ### Generics
/// - **Proprietor** - the entity (e.g. account, vault, or manager)
/// controlling the asset.
/// - **Pallet** - the public struct implementing [`Commitment`] traits
/// and [`LazyBalance`], ensuring consistent asset accounting across the
/// commitment system.
pub trait CommitOps<Proprietor, Pallet>
where
    Pallet: LazyBalance<
            Asset = <Pallet as InspectAsset<Proprietor>>::Asset,
            Variant = Pallet::Position,
            Id = Pallet::Digest,
        > + DigestModel<Proprietor>
        + CommitVariant<Proprietor>,
{
    /// Places a commitment using a fully resolved/wrapped digest model and
    /// a specified digest's balance variant.
    ///
    /// This function assumes the digest has already been found-valid and wrapped
    /// appropriately, so it directly handles the act of **only committing** the
    /// given value under the specified reason.
    ///
    /// It only defines **low-level, unchecked operations** - callers are
    /// responsible for ensuring validity and equilibrium before invoking this
    /// function.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the committed amount
    /// - `Err(DispatchError)` if the commitment fails
    fn place_commit_of(
        who: &Proprietor,
        reason: &Pallet::Reason,
        digest_model: &Pallet::Model,
        value: <Pallet as InspectAsset<Proprietor>>::Asset,
        variant: &Pallet::Position,
        qualifier: &Pallet::Intent,
    ) -> Result<<Pallet as InspectAsset<Proprietor>>::Asset, DispatchError>;

    /// Places a commitment for a **direct digest** under a given reason and variant.
    ///
    /// This function handles the placement of a commitment when the digest is a
    /// valid, single, direct item (not an index or pool). It records the committed
    /// value associated with the specific reason and digest, respecting the
    /// commitment rules.
    ///
    /// It only defines **low-level, unchecked operations** - callers are
    /// responsible for ensuring validity and equilibrium before invoking this
    /// function.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the committed amount
    /// - `Err(DispatchError)` if the commitment fails
    fn place_digest_commit(
        who: &Proprietor,
        reason: &Pallet::Reason,
        digest: &Pallet::Digest,
        value: <Pallet as InspectAsset<Proprietor>>::Asset,
        variant: &Pallet::Position,
        qualifier: &Pallet::Intent,
    ) -> Result<<Pallet as InspectAsset<Proprietor>>::Asset, DispatchError>;

    /// Places a commitment for a **index digest** under a given reason
    /// and variant.
    ///
    /// This function handles the placement of a commitment when the digest
    /// is a index. It records the committed value associated respecting
    /// the commitment rules - "One Reason, One Digest Commit" and uses high
    /// level structures to bypass it safely.
    ///
    /// It only defines **low-level, unchecked operations** - callers are
    /// responsible for ensuring validity and equilibrium before invoking this
    /// function.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the committed amount
    /// - `Err(DispatchError)` if the commitment fails
    fn place_index_commit(
        who: &Proprietor,
        reason: &Pallet::Reason,
        index_of: &Pallet::Digest,
        value: <Pallet as InspectAsset<Proprietor>>::Asset,
        variant: &Pallet::Position,
        qualifier: &Pallet::Intent,
    ) -> Result<<Pallet as InspectAsset<Proprietor>>::Asset, DispatchError>;

    /// Places a commitment for a **pool digest** under a given
    /// reason and variant.
    ///
    /// This function handles the placement of a commitment when
    /// the digest is a pool. It records the committed value associated
    /// respecting the commitment rules- "One Reason, One Digest Commit"
    /// and uses high level structures to bypass it safely.
    ///
    /// It only defines **low-level, unchecked operations** - callers are
    /// responsible for ensuring validity and equilibrium before invoking this
    /// function.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the committed amount
    /// - `Err(DispatchError)` if the commitment fails
    fn place_pool_commit(
        who: &Proprietor,
        reason: &Pallet::Reason,
        pool_of: &Pallet::Digest,
        value: <Pallet as InspectAsset<Proprietor>>::Asset,
        variant: &Pallet::Position,
        qualifier: &Pallet::Intent,
    ) -> Result<<Pallet as InspectAsset<Proprietor>>::Asset, DispatchError>;

    /// Raises an existing commitment by adding a new commitment
    /// instance.
    ///
    /// Commitments are immutable; each raise adds a new commit
    /// instance - by shareing semantics with existing commit of the reason.
    /// How accumulation is handled depends on the context: direct digest,
    /// index, or pool.
    ///
    /// It only defines **low-level, unchecked operations** - callers are
    /// responsible for ensuring validity and equilibrium before invoking this
    /// function.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the raised amount
    /// - `Err(DispatchError)` if the raise operation fails
    fn raise_commit_of(
        who: &Proprietor,
        reason: &Pallet::Reason,
        digest_model: &Pallet::Model,
        value: <Pallet as InspectAsset<Proprietor>>::Asset,
        qualifier: &Pallet::Intent,
    ) -> Result<<Pallet as InspectAsset<Proprietor>>::Asset, DispatchError>;

    /// Adds a new instance of a commitment for a direct digest.
    ///
    /// Commitments are immutable and cannot be changed once created. This
    /// enforces invariants by adding new commit instances to the same
    /// digest and reason.
    ///
    /// It only defines **low-level, unchecked operations** - callers are
    /// responsible for ensuring validity and equilibrium before invoking this
    /// function.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the raised amount
    /// - `Err(DispatchError)` if the raise operation fails
    fn raise_digest_commit(
        who: &Proprietor,
        reason: &Pallet::Reason,
        digest: &Pallet::Digest,
        value: <Pallet as InspectAsset<Proprietor>>::Asset,
        qualifier: &Pallet::Intent,
    ) -> Result<<Pallet as InspectAsset<Proprietor>>::Asset, DispatchError>;

    /// Adds a new instance of a commitment for an index digest.
    ///
    /// Commitments are immutable and cannot be changed once created. This
    /// enforces invariants by adding new commit instances to the same
    /// index digest and reason.
    ///
    /// It only defines **low-level, unchecked operations** - callers are
    /// responsible for ensuring validity and equilibrium before invoking this
    /// function.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the raised amount
    /// - `Err(DispatchError)` if the raise operation fails
    fn raise_index_commit(
        who: &Proprietor,
        reason: &Pallet::Reason,
        index_of: &Pallet::Digest,
        value: <Pallet as InspectAsset<Proprietor>>::Asset,
        qualifier: &Pallet::Intent,
    ) -> Result<<Pallet as InspectAsset<Proprietor>>::Asset, DispatchError>;

    /// Adds a new instance of a commitment for a pool digest.
    ///
    /// The pool should release and recover itself, adding the raised value.
    /// It may also record instances of commits for the proprietor to the pool rather
    /// than to its slot digest, since it collectively manages funds for commitments by
    /// the pool itself acting like a pseudo-proprietor.
    ///
    /// It only defines **low-level, unchecked operations** - callers are
    /// responsible for ensuring validity and equilibrium before invoking this
    /// function.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the raised amount
    /// - `Err(DispatchError)` if the raise operation fails
    fn raise_pool_commit(
        who: &Proprietor,
        reason: &Pallet::Reason,
        pool_of: &Pallet::Digest,
        value: <Pallet as InspectAsset<Proprietor>>::Asset,
        qualifier: &Pallet::Intent,
    ) -> Result<<Pallet as InspectAsset<Proprietor>>::Asset, DispatchError>;

    /// Resolves and finalizes a commitment for a fully resolved digest model.
    ///
    /// This method closes the commitment and provides the final withdrawn asset value
    /// from the commitment to the reason.
    ///
    /// It only defines **low-level, unchecked operations** - callers are
    /// responsible for ensuring validity and equilibrium before invoking this
    /// function.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the resolved commitment value
    /// - `Err(DispatchError)` if resolution fails
    fn resolve_commit_of(
        who: &Proprietor,
        reason: &Pallet::Reason,
        digest_model: &Pallet::Model,
    ) -> Result<<Pallet as InspectAsset<Proprietor>>::Asset, DispatchError>;

    /// Resolves and finalizes a commitment for a direct digest.
    ///
    /// This method closes the direct digest commitment and provides the
    /// final withdrawn asset value from the commitment to the reason.
    ///
    /// It only defines **low-level, unchecked operations** - callers are
    /// responsible for ensuring validity and equilibrium before invoking this
    /// function.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the resolved commitment value
    /// - `Err(DispatchError)` if resolution fails
    fn resolve_digest_commit(
        who: &Proprietor,
        reason: &Pallet::Reason,
        digest: &Pallet::Digest,
    ) -> Result<<Pallet as InspectAsset<Proprietor>>::Asset, DispatchError>;

    /// Resolves and finalizes a commitment for an index digest.
    ///
    /// This method closes the index digest commitment and provides the
    /// final withdrawn asset value from the commitment to the reason.
    ///
    /// It only defines **low-level, unchecked operations** - callers are
    /// responsible for ensuring validity and equilibrium before invoking this
    /// function.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the resolved commitment value
    /// - `Err(DispatchError)` if resolution fails
    fn resolve_index_commit(
        who: &Proprietor,
        reason: &Pallet::Reason,
        index_of: &Pallet::Digest,
    ) -> Result<<Pallet as InspectAsset<Proprietor>>::Asset, DispatchError>;

    /// Resolves and finalizes a commitment for a pool digest.
    ///
    /// This method closes the pool digest commitment and provides the
    /// final withdrawn asset value from the commitment to the reason.
    ///
    /// It only defines **low-level, unchecked operations** - callers are
    /// responsible for ensuring validity and equilibrium before invoking this
    /// function.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the resolved commitment value
    /// - `Err(DispatchError)` if resolution fails
    fn resolve_pool_commit(
        who: &Proprietor,
        reason: &Pallet::Reason,
        pool_of: &Pallet::Digest,
    ) -> Result<<Pallet as InspectAsset<Proprietor>>::Asset, DispatchError>;

    /// Sets the total asset value for a given reason.
    ///
    /// This is a **low-level function** and should only be used
    /// when equilibrium is maintained, meaning the value being set must
    /// correctly reflect the committed state of the system for the reason.
    fn set_total_value(reason: &Pallet::Reason, value: <Pallet as InspectAsset<Proprietor>>::Asset);

    /// Adds a given asset value to the total value for the specified reason.
    ///
    /// It only defines **low-level, unchecked operations** - callers are
    /// responsible for ensuring validity and equilibrium before invoking this
    /// function.
    ///
    /// ## Returns
    /// - `Ok(())` if the addition succeeds
    /// - `Err(DispatchError)` with `Overflow` if adding causes overflow
    fn add_to_total_value(
        reason: &Pallet::Reason,
        value: <Pallet as InspectAsset<Proprietor>>::Asset,
    ) -> DispatchResult {
        let current = Pallet::get_total_value(reason);
        let new_total = current
            .checked_add(&value)
            .ok_or(DispatchError::Arithmetic(ArithmeticError::Overflow))?;

        Self::set_total_value(reason, new_total);
        Ok(())
    }

    /// Subtracts a given asset value from the total value for the
    /// specified reason.
    ///
    /// It only defines **low-level, unchecked operations** - callers are
    /// responsible for ensuring validity and equilibrium before invoking
    /// this function.
    ///
    /// ### Returns
    /// - `Ok(())` if the subtraction succeeds
    /// - `Err(DispatchError)` with `Underflow` if subtraction would
    /// underflow
    fn sub_from_total_value(
        reason: &Pallet::Reason,
        value: <Pallet as InspectAsset<Proprietor>>::Asset,
    ) -> DispatchResult {
        let current = Pallet::get_total_value(reason);
        let new_total = current
            .checked_sub(&value)
            .ok_or(DispatchError::Arithmetic(ArithmeticError::Underflow))?;

        Self::set_total_value(reason, new_total);
        Ok(())
    }
}

// ===============================================================================
// ```````````````````````````````` COMMIT INSPECT ```````````````````````````````
// ===============================================================================

/// Provides inspection and querying capabilities for committed
/// values across digests, indexes, and pools for a given proprietor.
///
/// This trait allows retrieving **real-time committed values** without
/// altering state, supporting precise accounting and auditability.
///
/// This trait defines **low-level, unchecked operations** - callers are
/// responsible for ensuring validity and equilibrium before invoking these
/// functions.
///
/// ### Generics
/// - **Proprietor** - the entity (e.g. account, vault, or manager)
/// controlling the asset.
/// - **Pallet** - the public struct implementing [`Commitment`] traits
/// and [`LazyBalance`], ensuring consistent asset accounting across the
/// commitment system.
pub trait CommitInspect<Proprietor, Pallet>
where
    Pallet: LazyBalance<Asset = <Pallet as InspectAsset<Proprietor>>::Asset, Id = Pallet::Digest>
        + DigestModel<Proprietor>
        + Commitment<Proprietor>,
{
    /// Retrieves the real-time commitment value for a fully
    /// resolved digest model.
    ///
    /// Aggregates all individual commit instances associated with
    /// the digest model for the given proprietor and reason,
    /// returning a single live value.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the total committed value for the digest model
    /// - `Err(DispatchError)` if the value cannot be determined
    fn commit_value_of(
        who: &Proprietor,
        reason: &Pallet::Reason,
        digest_model: &Pallet::Model,
    ) -> Result<<Pallet as InspectAsset<Proprietor>>::Asset, DispatchError>;

    /// Retrieves the real-time commitment value for a direct digest.
    ///
    /// Returns the total value proprietor commit holds under the given
    /// reason for the direct digest.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the committed value for the digest
    /// - `Err(DispatchError)` if value cannot be retrieved
    fn digest_commit_value(
        who: &Proprietor,
        reason: &Pallet::Reason,
        digest: &Pallet::Digest,
    ) -> Result<<Pallet as InspectAsset<Proprietor>>::Asset, DispatchError>;

    /// Retrieves the real-time commitment value for an index digest.
    ///
    /// Computes the aggregate value across all entries contained
    /// in the index for the specified proprietor and reason.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the total committed value across
    /// all index entries
    /// - `Err(DispatchError)` if value cannot be calculated
    fn index_commit_value(
        who: &Proprietor,
        reason: &Pallet::Reason,
        index_of: &Pallet::Digest,
    ) -> Result<<Pallet as InspectAsset<Proprietor>>::Asset, DispatchError>;

    /// Retrieves the real-time commitment value for a specific
    /// entry within an index.
    ///
    /// Provides the real-time value for a single entry digest within
    /// the index, reflecting the proprietor's commitment to that
    /// specific entry.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the committed value for the entry
    /// - `Err(DispatchError)` if value cannot be retrieved
    fn index_entry_commit_value(
        who: &Proprietor,
        reason: &Pallet::Reason,
        index_of: &Pallet::Digest,
        entry_of: &Pallet::Digest,
    ) -> Result<<Pallet as InspectAsset<Proprietor>>::Asset, DispatchError>;

    /// Retrieves the real-time commitment value for
    /// a **pool digest**.
    ///
    /// Aggregates the values of all slots under the pool for
    /// the proprietor.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the total committed value
    /// across all pool slots
    /// - `Err(DispatchError)` if value cannot be calculated
    fn pool_commit_value(
        who: &Proprietor,
        reason: &Pallet::Reason,
        pool_of: &Pallet::Digest,
    ) -> Result<<Pallet as InspectAsset<Proprietor>>::Asset, DispatchError>;

    /// Retrieves the committed value of a specific slot for a
    /// particular proprietor.
    ///
    /// Aggregates the proprietor's commitment to that slot within the
    /// pool in real-time, showing their individual exposure to the slot.
    ///
    /// ### Returns
    /// - `Ok(Asset)` containing the proprietor's committed value for the slot
    /// - `Err(DispatchError)` if the slot does not exist or value cannot be
    /// retrieved
    fn pool_slot_commit_value(
        who: &Proprietor,
        reason: &Pallet::Reason,
        pool_of: &Pallet::Digest,
        slot_of: &Pallet::Digest,
    ) -> Result<<Pallet as InspectAsset<Proprietor>>::Asset, DispatchError>;

    /// Queries the real-time value of commitments for a given reason.
    ///
    /// This function calculates the current committed value based on the
    /// provided parameters:
    /// - If `digest_model` is provided, only values associated with that specific
    /// digest model are included
    /// - If `digest_model` is `None`, the total value across all relevant digests
    /// for the reason is returned
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the calculated total asset value
    /// - `Err(DispatchError)` if the query fails or the value cannot be determined
    fn value_of(
        digest_model: Option<&Pallet::Model>,
        reason: &Pallet::Reason,
    ) -> Result<<Pallet as InspectAsset<Proprietor>>::Asset, DispatchError>;
}

// ===============================================================================
// ``````````````````````````````` POOL OPERATIONS ```````````````````````````````
// ===============================================================================

/// Defines operational behavior for managing pooled balances within a commitment system.
///
/// A `PoolOps` implementation extends [`PoolVariant`] to provide concrete
/// management logic for a proprietor's pooled balances, where a pool may act
/// as a *single committing authority* maintaining aggregate or managed funds.
///
/// Unlike indexes (which represent discrete digests committed on behalf of a proprietor),
/// pools aggregate balances and resolve them collectively, acting as a unified
/// resource manager.
///
/// This trait defines **low-level, unchecked operations** - callers are
/// responsible for ensuring validity and equilibrium before invoking these
/// functions.
///
/// ### Generics
/// - **Proprietor** - the entity (e.g. account, vault, or manager)
/// controlling the asset.
/// - **Pallet** - the public struct implementing [`Commitment`] traits
/// and [`LazyBalance`], ensuring consistent asset accounting across the
/// commitment system.
pub trait PoolOps<Proprietor, Pallet>
where
    Pallet: LazyBalance<
            Asset = <Pallet as InspectAsset<Proprietor>>::Asset,
            Variant = Pallet::Position,
            Id = Pallet::Digest,
        > + PoolVariant<Proprietor>,
{
    /// The balance type associated with the pool.
    type PoolBalance;

    /// Releases a pool's balance, resetting it to its default state.
    ///
    /// Releasing a pool means resolving all active commit digests from this single balance,
    /// similar in concept to resolving a commit.
    ///
    /// - The release-recovery should be **immediate** at the calling site - safety must
    /// be ensured by the caller.
    /// - Pools differ from indexes: while indexes resolve multiple digests per proprietor,
    ///   pools resolve from a *single managed balance* representing aggregated commitments.
    ///
    /// It only defines **low-level, unchecked operations** - callers are
    /// responsible for ensuring validity and equilibrium before invoking this
    /// function.
    ///
    /// ## Returns
    /// - `Ok(PoolBalance)` containing the resolved pool balance
    /// - `Err(DispatchError)` if the release operation fails
    fn release_pool(
        reason: &Pallet::Reason,
        pool_of: &Pallet::Digest,
    ) -> Result<Self::PoolBalance, DispatchError>;

    /// Recovers a pool's state after a prior release via [`PoolOps::release_pool`].
    ///
    /// This function restores the pool's state following balance mutation or reconciliation.
    /// Should only be invoked after the pool has been released via [`PoolOps::release_pool`].
    ///
    /// It only defines **low-level, unchecked operations** - callers are
    /// responsible for ensuring validity and equilibrium before invoking this
    /// function.
    ///
    /// ## Returns
    /// - `Ok(())` if the pool state was successfully recovered
    /// - `Err(DispatchError)` if recovery fails
    fn recover_pool(
        reason: &Pallet::Reason,
        pool_of: &Pallet::Digest,
        balance: &Self::PoolBalance,
    ) -> DispatchResult;

    /// Removes a slot from a pool and updates the pool's state accordingly.
    ///
    /// This operation ensures that:
    /// - The slot represented by `slot_of` is removed from the pool.
    /// - The pool's collective balance is released and then recovered to reflect
    ///   the updated state after removal.
    ///
    /// Automatically updates the pool's internal representation of managed funds.
    ///
    /// It only defines **low-level, unchecked operations** - callers are
    /// responsible for ensuring validity and equilibrium before invoking this
    /// function.
    ///
    /// ### Returns
    /// - `Ok(())` if the slot was successfully removed and pool state updated
    /// - `Err(DispatchError)` if the operation fails
    fn remove_pool_slot(
        who: &Proprietor,
        reason: &Pallet::Reason,
        pool_of: &Pallet::Digest,
        slot_of: &Pallet::Digest,
    ) -> DispatchResult;

    /// Sets or updates a slot within a pool and synchronizes the pool's state.
    ///
    /// This operation may:
    /// - Add a new slot to the pool.
    /// - Update an existing slot's `shares` or `variant`.
    /// - Trigger a pool release and recovery cycle to ensure
    ///   the pool's balance reflects the new slot configuration.
    ///
    /// It only defines **low-level, unchecked operations** - callers are
    /// responsible for ensuring validity and equilibrium before invoking this
    /// function.
    ///
    /// ## Behavior
    /// - Ensures slot state consistency within the pool.
    /// - Used for slot creation, update, or reallocation of pool shares.
    ///
    /// ### Returns
    /// - `Ok(())` if the slot was successfully set and pool state synchronized
    /// - `Err(DispatchError)` if the operation fails
    fn set_pool_slot(
        who: &Proprietor,
        reason: &Pallet::Reason,
        pool_of: &Pallet::Digest,
        slot_of: &Pallet::Digest,
        shares: Pallet::Shares,
        variant: &Pallet::Position,
    ) -> DispatchResult;
}

// ===============================================================================
// ``````````````````````````````` INDEX OPERATIONS ``````````````````````````````
// ===============================================================================

/// Defines operational behavior for managing indexed digests within a commitment system.
///
/// An `IndexOps` implementation extends [`IndexVariant`] to provide concrete
/// management logic for a proprietor's indexed commitments, where an index acts
/// as a container of discrete digest entries.
///
/// Unlike pools (which aggregate balances collectively), indexes represent
/// *structured groupings* of digests that can be individually set or removed,
/// allowing for fine-grained commit management.
///
/// This trait defines **low-level, unchecked operations** - callers are
/// responsible for ensuring validity and equilibrium before invoking these
/// functions.
///
/// ### Generics
/// - **Proprietor** - the entity (e.g. account, vault, or manager)
/// controlling the asset.
/// - **Pallet** - the public struct implementing [`Commitment`] traits
/// and [`LazyBalance`], ensuring consistent asset accounting across the
/// commitment system.
pub trait IndexOps<Proprietor, Pallet>
where
    Pallet: LazyBalance<
            Asset = <Pallet as InspectAsset<Proprietor>>::Asset,
            Variant = Pallet::Position,
            Id = Pallet::Digest,
        > + IndexVariant<Proprietor>,
{
    /// Removes a digest entry from an index for the given proprietor and reason.
    ///
    /// Since indexes are immutable, this operation creates a new index without
    /// the specified entry and generates a new digest for it. The caller is
    /// responsible for managing the lifecycle of the old index digest.
    ///
    /// This is a **low-level, unchecked operation** - callers must ensure
    /// equilibrium and invariant safety before calling.
    ///
    /// ## Returns
    /// - `Ok(Digest)` containing the new index digest after entry removal
    /// - `Err(DispatchError)` if the operation fails
    fn remove_index_entry(
        who: &Proprietor,
        reason: &Pallet::Reason,
        index_of: &Pallet::Digest,
        entry_of: &Pallet::Digest,
    ) -> Result<Pallet::Digest, DispatchError>;

    /// Sets or updates a digest entry within an index and returns the new index digest.
    ///
    /// Since indexes are immutable, this operation creates a new index with the updated
    /// entry configuration (shares and variant) and generates a new digest for it.
    /// If the entry exists, it is updated; otherwise, it is added.
    ///
    /// This is a **low-level, unchecked operation** - callers must ensure
    /// equilibrium and invariant safety before calling.
    ///
    /// ## Returns
    /// - `Ok(Digest)` containing the new index digest after entry modification
    /// - `Err(DispatchError)` if the operation fails
    fn set_index_entry(
        who: &Proprietor,
        reason: &Pallet::Reason,
        index_of: &Pallet::Digest,
        entry_of: &Pallet::Digest,
        shares: Pallet::Shares,
        variant: &Pallet::Position,
    ) -> Result<Pallet::Digest, DispatchError>;
}
