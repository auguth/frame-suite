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
// ``````````````````````````` AUTHORS ROLE MANAGEMENT ```````````````````````````
// ===============================================================================

//! Provides the **concrete runtime implementation** of the Author subsystem,
//! translating abstract [`role traits`](frame_suite::roles) into operational
//! logic governing authors' funding, rewards/penalties, and lifecycle
//! within the runtime.
//!
//! ## Purpose
//!
//! Authors are key participants whose behavior, backing, and status directly affect
//! **network security and correctness**. While the role traits define *what* is required
//! of authors, this module defines *how* those requirements are enforced in a
//! **safe, predictable, and auditable** manner.
//!
//! ## Role Implementations
//!
//! ### 1. Funding ([`FundRoles<Author>`])
//!
//! - Enables authors to operate with external economic backing, ensuring
//!   **skin-in-the-game** and accountability.
//! - Protects backers by enforcing correct fund allocation via digests and
//!   commitment checks ([`Commitment`]).
//! - Supports multiple funding models (direct, index-based, pooled) while
//!   maintaining a **uniform, auditable interface** through the
//! [`Config::CommitmentAdapter`].
//!
//! ### 2. Compensation ([`CompensateRoles<Author>`])
//!
//! - Aligns author incentives through rewards and penalties.
//! - Enforces **temporal separation**: obligations are scheduled for future blocks
//!   to ensure deterministic execution and prevent immediate exploitation.
//! - Preserves **hold consistency**, ensuring collateral and external funds are always
//!   accurately reflected in an author's total hold.
//!
//! ### 3. Probation & Permanence ([`RoleProbation<Author>`])
//!
//! - Prevents immediate permanence by enforcing a **probation window** for behavioral
//!   observation.
//! - Allows authors to be marked **temporarily unsafe** without irreversible removal,
//!   enabling adaptive risk management.
//! - Ensures clear promotion and revocation rules, preventing bypass of safety
//!   invariants or accountability mechanisms.
//!
//! By concretely implementing these role traits, this module transforms abstract role
//! definitions into **runtime-safe, economically secure, and auditable author governance**.

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- Local crate imports ---
use crate::types::*;
use crate::*;

// --- FRAME Suite ---
use frame_suite::{commitment::*, roles::*, Directive, Extent};

// --- FRAME Support ---
use frame_support::{
    dispatch::DispatchResult,
    ensure,
    traits::tokens::{Fortitude, Precision},
};

// --- FRAME System ---
use frame_system::pallet_prelude::*;

// --- Substrate primitives ---
use sp_core::Get;
use sp_runtime::{
    traits::{Bounded, CheckedAdd, Saturating, Zero},
    DispatchError, PerThing, Perbill, Vec,
};

// ===============================================================================
// ```````````````````````````````` ROLE MANAGER `````````````````````````````````
// ===============================================================================

/// Implements the [`RoleManager`] trait for the **Author subsystem**
///
/// Defines how authors behave as *role-bearing entities* within
/// the runtime.  
impl<T: Config> RoleManager<Author<T>> for Pallet<T> {
    /// The possible states of an `Author` role.
    ///
    /// Variants:
    /// - `Active`      : Author is actively participating.
    /// - `Probation`   : Author is under review or subject to restrictions.
    /// - `Resigned`    : Author voluntarily left the role.
    ///
    /// Note: There is no explicit suspension; penalties and probation are
    /// applied to enforce decentralization.
    type Status = AuthorStatus;

    /// The meta-information of an `Author` role.
    type Meta = AuthorInfo<T>;

    /// The type representing the collateral or hold of an `Author` role.
    type Asset = AuthorAsset<T>;

    /// Timestamp type used for enrollment or status tracking.
    type TimeStamp = BlockNumberFor<T>;

    /// Checks whether the given `Author` exists in the system.
    ///
    /// Returns:
    /// - `Ok(())` if the author exists.
    /// - `Err(DispatchError)` otherwise.
    fn role_exists(who: &Author<T>) -> DispatchResult {
        ensure!(
            AuthorsMap::<T>::contains_key(who),
            Error::<T>::AuthorNotFound
        );
        Ok(())
    }

    /// Retrieves the meta-data of the given `Author` if available.
    ///
    /// Returns:
    /// - `Ok(Meta)` if the author exists.
    /// - `Err(DispatchError)` otherwise.
    fn get_meta(who: &Author<T>) -> Result<Self::Meta, DispatchError> {
        let info = AuthorsMap::<T>::get(who).ok_or(Error::<T>::AuthorNotFound)?;
        Ok(info)
    }

    /// Retrieves the amount of collateral currently locked by an `Author` during
    /// enrollment.
    ///
    /// This ensures real-time accuracy, reflecting any updates to the collateral.
    ///
    /// - Does not check author validaity, since commitment call reflects
    /// if the pallet-gated collateral reason is funded by the given author
    /// - Invariant: [`FreezeReason::AuthorCollateral`] must only be utilized by this pallet
    /// - Invariant: Ensures the collateral must be non-zero, or else most of the functions will
    /// fail.
    ///
    /// Returns the collateral value or a `DispatchError` otherwise.
    fn get_collateral(who: &Author<T>) -> Result<Self::Asset, DispatchError> {
        let reason = &FreezeReason::AuthorCollateral.into();
        let value = T::CommitmentAdapter::get_commit_value(who, reason)?;
        Ok(value)
    }

    /// Retrieves the amount of collateral currently locked by all `Author`s during
    /// enrollment.
    ///
    /// This ensures real-time accuracy, reflecting any updates to any collaterals.
    fn total_collateral() -> Self::Asset {
        let reason = &FreezeReason::AuthorCollateral.into();
        T::CommitmentAdapter::get_total_value(reason)
    }

    /// Returns the block number when the `Author` enrolled in the role.
    ///
    /// DispatchError otherwise
    fn enroll_since(who: &Author<T>) -> Result<Self::TimeStamp, DispatchError> {
        let info = Self::get_meta(who)?;
        Ok(info.since)
    }

    /// Retrieves the current status of the given `Author`.
    ///
    /// Status can be one of:
    /// - `Active`
    /// - `Probation`
    /// - `Resigned`
    ///
    /// DispatchError otherwise
    fn get_status(who: &Author<T>) -> Result<Self::Status, DispatchError> {
        let info = Self::get_meta(who)?;
        Ok(info.status)
    }

    /// Returns the timestamp (block number) when the author's current status was last updated.
    ///
    /// This can be used to track how long an author has been in a specific state
    /// (e.g., probation, active, resigned) and enforce time-based rules.
    ///
    /// DispatchError otherwise.
    fn status_since(who: &Author<T>) -> Result<Self::TimeStamp, DispatchError> {
        let info = Self::get_meta(who)?;
        return Ok(info.status_since);
    }

    /// Updates the status of an author in a **safe, controlled way**.
    ///
    /// It doesn't mutate status directly, but enforces validations to proceed.
    ///
    /// DispatchError otherwise.
    fn set_status(who: &Author<T>, status: Self::Status) -> DispatchResult {
        let info = Self::get_meta(who)?;
        let current_status = info.status;
        match current_status {
            // Current status: Active
            AuthorStatus::Active => match status {
                // No-op if status unchanged
                AuthorStatus::Active => {}
                // Try sending active author to probation
                AuthorStatus::Probation => {
                    Self::revoke_permanence(who)?;
                }
                // Trigger full resignation workflow
                AuthorStatus::Resigned => {
                    // But cannot return the regained asset
                    // Ensure `resign` doesn't use `set_status`
                    // to avoid indefinite recursion
                    Self::resign(who)?;
                }
            },
            // Current status: Probation
            AuthorStatus::Probation => match status {
                // No-op if unchanged
                AuthorStatus::Probation => {}
                // Cannot resign during probation
                AuthorStatus::Resigned => return Err(Error::<T>::AuthorInProbation.into()),
                AuthorStatus::Active => {
                    Self::set_permanence(who)?;
                }
            },
            // Current status: Resigned
            AuthorStatus::Resigned => match status {
                AuthorStatus::Active | AuthorStatus::Probation => {
                    // Resigned authors cannot be reactivated via `set_status` directly,
                    // only via `enroll` it can be done
                    return Err(Error::<T>::AuthorResigned.into());
                }
                // No-op if unchanged
                AuthorStatus::Resigned => {}
            },
        }
        Self::on_status_update(who, &status);
        Ok(())
    }

    /// Validates whether an `Author` can enroll with the given collateral.
    ///
    /// Checks include:
    /// - If the status is `Resigned`, enrollment is allowed (re-entry).
    /// - Ensures the provided collateral meets the minimum requirement.
    ///
    /// Returns:
    /// - `Ok(())` if all checks pass.
    /// - `Err(DispatchError) otheriwse.
    fn can_enroll(who: &Author<T>, collateral: Self::Asset) -> DispatchResult {
        // In case of re-enrollment by resigned authors
        if Self::role_exists(who).is_ok() {
            let status = Self::get_status(who);
            debug_assert!(
                status.is_ok(),
                "author {:?} role-exists but status unavailable",
                who
            );
            match status? {
                AuthorStatus::Resigned => {
                    // Resigned must not have any penalties (obligations)
                    debug_assert!(
                        Self::has_penalty(who).is_err(),
                        "author {:?} resigned with penalty and attempting re-enrollment",
                        who
                    );

                    // In this case, the author has regained his collateral
                    // irrespective of rewards,
                    // hence he cannot claim it but if there are funders
                    // they can claim it.
                    if Self::has_reward(who).is_ok() {
                        return Err(Error::<T>::AuthorHasRewards.into());
                    }
                }
                AuthorStatus::Active | AuthorStatus::Probation => {
                    return Err(Error::<T>::AlreadyEnrolled.into())
                }
            }
        }
        let min_collateral = MinCollateral::<T>::get();
        debug_assert!(
            !min_collateral.is_zero(),
            "`MinCollateral` must be greater than zero"
        );
        let available = T::CommitmentAdapter::available_funds(who);
        // Ensure collateral funds are available
        ensure!(!(available < collateral), Error::<T>::InadequateFunds);
        // Ensure minimum collateral requirement is met
        ensure!(
            !(collateral < min_collateral),
            Error::<T>::InadequateCollateral
        );
        Ok(())
    }

    /// Enrolls a new author with the specified collateral and the operation's
    /// priviledge via `force`.
    ///
    /// Steps performed:
    /// - Ensure the author is eligible and the collateral meets minimum requirements.
    /// - Generate a unique digest/hash for this author's funding commitment.
    /// - Lock the collateral using the commitment adapter.
    /// - Register the author in storage and maintain lookup maps.
    ///
    /// For Resigned Authors Enrollment, their commitment-digest is reused.
    ///
    /// ## `force` Semantics
    /// - [`Fortitude::Polite`]: Uses funds from the **commitment reserve**.
    /// - [`Fortitude::Force`]: Uses funds from the user's **liquid balance**.
    ///
    /// Prefer `Polite` when collateral is pre-reserved; otherwise use `Force`.
    ///
    /// This operation will **never kill an account**, as guaranteed by the
    /// commitment system.
    ///
    /// ## Errors
    /// - Returns the actual amount of collateral successfully reserved.
    /// - Returns a `DispatchError` if fails.
    fn enroll(
        who: &Author<T>,
        collateral: Self::Asset,
        force: Fortitude,
    ) -> Result<Self::Asset, DispatchError> {
        //  Validate enrollment eligibility,
        // also checks for resigned authors
        Self::can_enroll(who, collateral)?;

        // Safe enrollment for resigned authors
        let (meta, digest) = match Self::role_exists(who).is_ok() {
            true => {
                let meta = Self::get_meta(who);
                debug_assert!(
                    meta.is_ok(),
                    "author {:?} role-exists but meta unavailable",
                    who
                );
                let meta = meta?;
                debug_assert!(
                    AuthorStatus::Resigned == meta.status,
                    "re-enroll tried for non-resigned author {:?}",
                    who
                );
                let info = AuthorInfo::<T>::re_enroll(&meta);
                // This may change if new feature of disincentivizing casual resignations
                // are introduced
                debug_assert!(
                    meta.digest == info.digest,
                    "resigned author {:?} re-enroll tried with new commit digest",
                    who
                );
                let digest = meta.digest;
                (info, digest)
            }
            false => {
                // Generate a unique digest for this author's collateral commitment
                let digest = T::CommitmentAdapter::gen_digest(who)
                    .map_err(|_| Error::<T>::CannotGenerateCommitDigest)?;
                let info = AuthorInfo::<T>::new(digest.clone());
                (info, digest)
            }
        };

        let reason = &FreezeReason::AuthorCollateral.into();

        let limits = T::CommitmentAdapter::place_commit_limits(
            who,
            reason,
            &digest,
            &Directive::new(
                Precision::Exact, // Enforce exact collateral placement
                force,
            ),
        )?;

        let actual = match limits.contains(collateral) {
            true => {
                // Place the collateral in the commitment system
                T::CommitmentAdapter::place_commit(
                    who,
                    reason,
                    &digest,
                    collateral,
                    &Directive::new(
                        Precision::Exact, // Enforce exact collateral placement
                        force,
                    ),
                )?
            }
            false => {
                // Place the minimum-collateral in the commitment system
                T::CommitmentAdapter::place_commit(
                    who,
                    reason,
                    &digest,
                    MinCollateral::<T>::get(), // Enforce minimum collateral placement
                    &Directive::new(Precision::Exact, force),
                )?
            }
        };

        // Register the author in pallet storage
        AuthorsMap::<T>::insert(who, &meta);

        AuthorsDigest::<T>::insert(&digest, who);

        Self::on_enroll(who, collateral);

        // Return the amount of collateral actually reserved
        Ok(actual)
    }

    /// Validates whether an `Author` can safely resign from the role.
    ///
    /// Checks include:
    /// - Author in `Probation` cannot resign (must resolve probation first).
    /// - Ensures there are no pending penalties.
    /// - Ensures the author is not currently active in duties (cannot resign while active).
    ///
    /// Pending rewards are ignored, as its voluntary for author to resign before
    /// receiving the rewards, whereas the backers are unaffected for receiving.
    ///
    /// Returns:
    /// - `Ok(())` if all conditions are met.
    /// - `Err(DispatchError) otherwise.
    fn can_resign(who: &Author<T>) -> DispatchResult {
        let status = Self::get_status(who)?;
        // Find Non-Resignable Statuses
        match status {
            AuthorStatus::Probation => return Err(Error::<T>::AuthorInProbation.into()),
            AuthorStatus::Resigned => return Err(Error::<T>::RedundantResignation.into()),
            AuthorStatus::Active => {}
        }
        // Check for any pending penalties
        if Self::has_penalty(who).is_ok() {
            return Err(Error::<T>::AuthorHasPenalties.into());
        }
        // Ensure author is currently idle (not active)
        if let Err(a) = T::ActivityProvider::is_idle(who) {
            return Err(a.into());
        };

        Ok(())
    }

    /// Resigns an author, releasing collateral and updating status.
    ///
    /// Marks author's status as `Resigned` (so funders may withdraw their funds later).
    ///
    /// If an author's metadata is ever reaped, a *separate, safety-checked procedure*
    /// MUST ensure that **all funders have fully withdrawn their commitments**.
    /// Only once this invariant holds it is safe to issue a new digest and purge
    /// the old entry from [`AuthorsDigest`] during re-enrollment.
    ///
    /// **This function does not perform those checks and MUST NOT be used for that purpose.**
    ///
    /// Returns the refunded collateral of the author. DispatchError otherwise.
    fn resign(who: &Author<T>) -> Result<Self::Asset, DispatchError> {
        // Ensure author can resign
        Self::can_resign(who)?;

        // Does not reaps the maps as its duty should live elsewhere for safety
        AuthorsMap::<T>::mutate(who, |author| -> DispatchResult {
            let info = author.as_mut();
            debug_assert!(
                info.is_some(),
                "author {:?} can-resign without its author-info",
                who
            );
            let info = info.ok_or(Error::<T>::AuthorNotFound)?;
            let status = &mut info.status;
            *status = AuthorStatus::Resigned;
            Ok(())
        })?;

        // Only withdraw the collateral for the author
        // Funders may withdraw at their own convenience
        let reason = &FreezeReason::AuthorCollateral.into();

        // Release the collateral the author provided.
        let refund = T::CommitmentAdapter::resolve_commit(who, reason)?;

        Self::on_resign(who, refund);
        Ok(refund)
    }

    /// Increases the collateral for an Author with the specified collateral
    /// and the operation's priviledge (`force`).
    ///
    /// If the existing collateral (before raising/adding) is lesser than the system enforced
    /// minimum, this function uses [`Precision::Exact`] else [`Precision::BestEffort`].
    ///
    /// `force` determines the source of funds:
    /// - [`Fortitude::Polite`]: commitment reserve
    /// - [`Fortitude::Force`]: liquid balance
    ///
    /// Returns the actually raised collateral (not full collateral). DispatchError otherwise.
    fn add_collateral(
        who: &Author<T>,
        collateral: Self::Asset,
        force: Fortitude,
    ) -> Result<Self::Asset, DispatchError> {
        let exist_collateral = Self::get_collateral(who)?;
        let reason = &FreezeReason::AuthorCollateral.into();
        let minimum = MinCollateral::<T>::get();
        debug_assert!(
            !minimum.is_zero(),
            "`MinCollateral` must be greater than zero"
        );
        let raised = match exist_collateral < minimum {
            true => T::CommitmentAdapter::raise_commit(
                who,
                reason,
                collateral,
                &Directive::new(Precision::Exact, force),
            )?,
            false => T::CommitmentAdapter::raise_commit(
                who,
                reason,
                collateral,
                &Directive::new(Precision::BestEffort, force),
            )?,
        };
        Self::on_add_collateral(who, raised);
        Ok(raised)
    }

    /// Checks if the author is not defaulted (available).
    ///
    /// - Active or Probation authors are not considered defaulted (returns error).
    /// - Resigned authors are treated as defaulted.
    /// - Lesser Collateral will result in author being defaulted.
    fn is_available(who: &Author<T>) -> DispatchResult {
        let status = Self::get_status(who)?;
        if status == AuthorStatus::Resigned {
            return Err(Error::<T>::AuthorResigned.into());
        }
        let collateral = Self::get_collateral(who);
        debug_assert!(
            collateral.is_ok(),
            "author {:?} with status exist without a collateral",
            who
        );
        let min_collateral = MinCollateral::<T>::get();
        debug_assert!(
            !min_collateral.is_zero(),
            "`MinCollateral` must be greater than zero"
        );
        if collateral? < min_collateral {
            return Err(Error::<T>::AuthorNeedsMoreCollateral.into());
        }
        Ok(())
    }

    /// Hook invoked after an author is successfully enrolled.
    ///
    /// Emits [`Event::AuthorEnlisted`] if [`Config::EmitEvents`] is `true`.
    fn on_enroll(who: &Author<T>, collateral: Self::Asset) {
        if T::EmitEvents::get() {
            Self::deposit_event(Event::<T>::AuthorEnlisted {
                author: who.clone(),
                collateral,
            });
        }
    }

    /// Hook invoked when an author resignation is processed.
    ///
    /// Emits [`Event::AuthorResigned`] if [`Config::EmitEvents`] is `true`.
    fn on_resign(who: &Author<T>, released: Self::Asset) {
        if T::EmitEvents::get() {
            Self::deposit_event(Event::<T>::AuthorResigned {
                author: who.clone(),
                released,
            });
        }
    }

    /// Hook invoked after an author's collateral balance is incremented.
    ///
    /// Emits [`Event::AuthorCollateralRaised`] if [`Config::EmitEvents`] is `true`.
    fn on_add_collateral(who: &Author<T>, raised: Self::Asset) {
        if T::EmitEvents::get() {
            Self::deposit_event(Event::<T>::AuthorCollateralRaised {
                author: who.clone(),
                raised,
            });
        }
    }

    /// Hook invoked after an author's status is mutated or updated.
    ///
    /// Emits [`Event::AuthorStatus`] if [`Config::EmitEvents`] is `true`.
    fn on_status_update(who: &Author<T>, status: &Self::Status) {
        if T::EmitEvents::get() {
            Self::deposit_event(Event::<T>::AuthorStatus {
                author: who.clone(),
                status: status.clone(),
            });
        }
    }
}

// ===============================================================================
// ````````````````````````````````` FUND ROLES ``````````````````````````````````
// ===============================================================================

/// Implements the [`FundRoles`] trait for the **Author subsystem**
///
/// Defines how authors can be externally backed by external collaterals
/// within the runtime.  
impl<T: Config> FundRoles<Author<T>> for Pallet<T> {
    /// Represents the entity providing funding to an author.
    ///
    /// Can be a direct account, an index, or a managed pool.
    ///
    /// Indirect backers such as index or pools must have a direct account
    /// willing to back (fund) it.
    type Backer = Funder<T>;

    /// Checks if the author has any active backers/funds.
    ///
    /// Returns a DispatchError if no funders exist.
    fn has_funds(who: &Author<T>) -> DispatchResult {
        let Some(_) = AuthorFunders::<T>::iter_prefix((who,)).next() else {
            return Err(Error::<T>::FundDoesNotExist.into());
        };
        Ok(())
    }

    /// Returns the **maximum exposure** allowed for an [`Author`] from a `Backer`,
    /// under the directive of the attempted funding.
    ///
    /// For [`Funder::Direct`] backers, limits are derived from:
    /// - global constraint ([`MaxExposure`]),
    /// - author-specific constraint ([`AuthorInfo`]'s `max_fund`),
    /// - and underlying commitment limits via [`CommitmentAdapter`][Config::CommitmentAdapter],
    ///   depending on whether the fund is new or being raised.
    ///
    /// For index and pool funders:
    /// - only global and author-specific constraints are applied.
    ///
    /// The `precision` and `force` parameters simulate the directive of
    /// the funding attempt.
    fn max_exposure(
        by: &Self::Backer,
        to: &Author<T>,
        precision: Precision,
        force: Fortitude,
    ) -> Result<Self::Asset, DispatchError> {
        let info = Self::get_meta(to)?;
        let global = MaxExposure::<T>::get();

        debug_assert!(
            global >= MinFund::<T>::get(),
            "`MaxExposure` must be greater than or equal to `MinFund`"
        );

        // Local (author-specific) constraint
        let local = info.max_fund.unwrap_or(global);

        let base_max = local.min(global);

        let Funder::Direct(funder) = by else {
            return Ok(base_max);
        };

        // ---- Commitment-aware limits ----

        let reason = &FreezeReason::AuthorFunding.into();
        let directive = &Directive::new(precision, force);

        let author_digest = &info.digest;

        let commit_exists = T::CommitmentAdapter::commit_exists(funder, reason).is_ok();

        let limits = match commit_exists {
            true => {
                let exist_digest = T::CommitmentAdapter::get_commit_digest(funder, reason)?;
                ensure!(
                    exist_digest == *author_digest,
                    Error::<T>::FundedToAnotherDigest
                );
                T::CommitmentAdapter::raise_commit_limits(funder, reason, directive)?
            }
            false => {
                T::CommitmentAdapter::place_commit_limits(funder, reason, author_digest, directive)?
            }
        };

        let commit_max = limits.maximum().unwrap_or(Bounded::max_value());

        // Final max = min(local/global, commitment)
        Ok(base_max.min(commit_max))
    }

    /// Returns the **minimum funding amount** required for a `Backer` to fund
    /// an [`Author`], under the directive of the attempted funding.
    ///
    /// For [`Funder::Direct`] backers, limits are derived from:
    /// - global constraint ([`MinFund`]),
    /// - author-specific constraint ([`AuthorInfo`]'s `min_fund`),
    /// - and underlying commitment limits via [`CommitmentAdapter`](Config::CommitmentAdapter),
    ///   depending on whether the fund is new or being raised.
    ///
    /// For index and pool funders:
    /// - only global and author-specific constraints are applied.
    ///
    /// The `precision` and `force` parameters simulate the directive of
    /// the funding attempt.
    fn min_fund(
        by: &Self::Backer,
        to: &Author<T>,
        precision: Precision,
        force: Fortitude,
    ) -> Result<Self::Asset, DispatchError> {
        let info = Self::get_meta(to)?;
        let global = MinFund::<T>::get();
        debug_assert!(!global.is_zero(), "`MinFund` must be greater than zero");
        debug_assert!(
            global <= MaxExposure::<T>::get(),
            "`MinFund` must be smaller than or equal to `MaxExposure`"
        );

        // Local (author-specific) constraint
        let local = info.min_fund.unwrap_or(global);

        let base_min = local.max(global);

        let Funder::Direct(funder) = by else {
            return Ok(base_min);
        };

        // ---- Commitment-aware limits ----

        let reason = &FreezeReason::AuthorFunding.into();
        let directive = &Directive::new(precision, force);

        let author_digest = &info.digest;

        let commit_exists = T::CommitmentAdapter::commit_exists(funder, reason).is_ok();

        let limits = match commit_exists {
            true => {
                let exist_digest = T::CommitmentAdapter::get_commit_digest(funder, reason)?;
                ensure!(
                    exist_digest == *author_digest,
                    Error::<T>::FundedToAnotherDigest
                );
                T::CommitmentAdapter::raise_commit_limits(funder, reason, directive)?
            }
            false => {
                T::CommitmentAdapter::place_commit_limits(funder, reason, author_digest, directive)?
            }
        };

        let commit_min = limits.minimum().unwrap_or(Zero::zero());

        // Final min = max(local/global, commitment)
        Ok(base_min.max(commit_min))
    }

    /// Total real-time funds currently backing the author (excluding the author's own collateral).
    ///
    /// Only includes finalized commitments; pending rewards or penalties are ignored.
    fn backed_value(who: &Author<T>) -> Result<Self::Asset, DispatchError> {
        let info = Self::get_meta(who)?;
        let reason = &FreezeReason::AuthorFunding.into();
        let value = T::CommitmentAdapter::get_digest_value(reason, &info.digest)?;
        Ok(value)
    }

    /// Total real-time funds currently backing **all the authors** (excluding all authors own collaterals).
    ///
    /// Only includes finalized commitments; pending rewards or penalties are ignored.
    fn total_backing() -> Self::Asset {
        let reason = &FreezeReason::AuthorFunding.into();
        T::CommitmentAdapter::get_total_value(reason)
    }

    /// Validates whether a backer can fund a given author.
    ///
    /// Returns `Ok(())` if the backer is eligible to fund, or a `DispatchError` otherwise.
    fn can_fund(
        by: &Self::Backer,
        to: &Author<T>,
        value: Self::Asset,
        precision: Precision,
        force: Fortitude,
    ) -> DispatchResult {
        // Ensure author is available (not defaulted)
        Self::is_available(to)?;

        let info = Self::get_meta(to);
        debug_assert!(
            info.is_ok(),
            "author {:?} is-available without its meta",
            to
        );
        let info = info?;
        let author_digest = &info.digest;
        let reason = &FreezeReason::AuthorFunding.into();

        let (funder, towards) = match by {
            Funder::Direct(backer) => {
                // Fund value range check within limits defined by author/global/commitment
                ensure!(
                    value >= Self::min_fund(by, to, precision, force)?,
                    Error::<T>::BelowMinimumFund
                );
                ensure!(
                    value <= Self::max_exposure(by, to, precision, force)?,
                    Error::<T>::AboveMaximumExposure
                );
                (backer, author_digest)
            }
            Funder::Index { digest, backer } => {
                // Fund value range check within limits defined by global
                ensure!(value >= MinFund::<T>::get(), Error::<T>::BelowMinimumFund);
                ensure!(
                    value <= MaxExposure::<T>::get(),
                    Error::<T>::AboveMaximumExposure
                );
                // Check if the author's digest available in the index entries
                T::CommitmentAdapter::entry_exists(reason, digest, author_digest)?;
                (backer, digest)
            }
            Funder::Pool { digest, backer } => {
                // Fund value range check within limits defined by global
                ensure!(value >= MinFund::<T>::get(), Error::<T>::BelowMinimumFund);
                ensure!(
                    value <= MaxExposure::<T>::get(),
                    Error::<T>::AboveMaximumExposure
                );
                // Check if the author's digest available in the pool slots
                T::CommitmentAdapter::slot_exists(reason, digest, author_digest)?;
                (backer, digest)
            }
        };

        // In case if its not the first funding commitment for the funder (signing backer)
        if T::CommitmentAdapter::commit_exists(funder, reason).is_ok() {
            let exist_digest = T::CommitmentAdapter::get_commit_digest(funder, reason)?;
            ensure!(exist_digest == *towards, Error::<T>::FundedToAnotherDigest);
        }
        Ok(())
    }

    /// Funds an author on behalf of a backer.
    ///
    /// This function either places a new fund for an author or increases an existing fund
    /// if the author has already been funded by the same backer.
    ///
    /// The backer [`Self::Backer`] is of type [`Funder`] in itself explains to
    /// whom its funding, and via what.
    ///
    /// This function asks a suitable author `to` diligently even for index and pool backers.
    /// It is never unused as we can do additional invariant checks.
    ///
    /// Hence indexes and pool backers should ensure that the author given is indeed true
    /// in its context of valdidation i.e., author available in respective entires or slots.
    ///
    /// In case of backers being index or pools, the returned amount reflects the
    /// total funding to it (may not be only for the given author)
    ///
    /// ## Returns
    /// - `Ok(Asset)`: The amount successfully funded.
    /// - `Err(DispatchError)` otherwise.
    fn fund(
        to: &Author<T>,
        by: &Self::Backer,
        value: Self::Asset,
        precision: Precision,
        force: Fortitude,
    ) -> Result<Self::Asset, DispatchError> {
        // Validate that the backer can fund the author with the specified value
        Self::can_fund(by, to, value, precision, force)?;

        // Reason used for freezing/funding the commitment
        let reason = &FreezeReason::AuthorFunding.into();

        let info = Self::get_meta(to);
        debug_assert!(
            info.is_ok(),
            "backer can-fund but given author's {:?} meta not available",
            to
        );
        let info = info?;
        let author_digest = &info.digest;

        // Determine funder and target digest based on the backer type
        let (funder, towards) = match by {
            Funder::Direct(backer) => (backer, author_digest),
            Funder::Index { digest, backer } => (backer, digest),
            Funder::Pool { digest, backer } => (backer, digest),
        };

        // If a commitment already exists, raise it; otherwise, place a new commitment
        let actual;
        match T::CommitmentAdapter::commit_exists(funder, reason) {
            Ok(_) => {
                actual = T::CommitmentAdapter::raise_commit(
                    funder,
                    reason,
                    value,
                    &Directive::new(precision, force),
                )?;
            }
            Err(_) => {
                actual = T::CommitmentAdapter::place_commit(
                    funder,
                    reason,
                    towards,
                    value,
                    &Directive::new(precision, force),
                )?;
            }
        }

        // Update the funders of author
        // In case of index backer or a pool, all the funded authors must get reflected on their recent funding
        match by {
            Funder::Direct(_) => {
                AuthorFunders::<T>::insert((to, funder), &by);
            }
            Funder::Index { digest, backer: _ } => {
                let entries = T::CommitmentAdapter::get_entries_shares(reason, digest)?;
                for (entry, _) in entries {
                    let author = AuthorsDigest::<T>::get(&entry);
                    let author = author.ok_or(Error::<T>::AuthorDigestNotFound)?;
                    AuthorFunders::<T>::insert((&author, funder), by);
                }
            }
            Funder::Pool { digest, backer: _ } => {
                let slots = T::CommitmentAdapter::get_slots_shares(reason, digest)?;
                for (slot, _) in slots {
                    let author = AuthorsDigest::<T>::get(&slot);
                    let author = author.ok_or(Error::<T>::AuthorDigestNotFound)?;
                    AuthorFunders::<T>::insert((&author, funder), by);
                }
            }
        }
        Self::on_funded(to, by, actual);
        Ok(actual)
    }

    /// Validates whether a backer can withdraw their existing fund from a given author.
    ///
    /// The backer [`Self::Backer`] is of type [`Funder`] in itself explains to
    /// whom its funded, and via what.
    ///
    /// This function asks a suitable author `from` diligently even for index and pool backers.
    /// It is never unused as we can do additional invariant checks.
    ///
    /// Hence indexes and pool backers should ensure that the author given is indeed true
    /// in its context of valdidation i.e., author available in respective entires or slots.
    ///
    /// Returns `Ok(())` if the backer is eligible to withdraw the fund, or a `DispatchError` otherwise.
    fn can_draw(by: &Self::Backer, from: &Author<T>) -> DispatchResult {
        let info = Self::get_meta(from)?;
        let author_digest = &info.digest;
        let reason = &FreezeReason::AuthorFunding.into();

        let (funder, towards) = match by {
            Funder::Direct(backer) => (backer, author_digest),
            Funder::Index { digest, backer } => {
                // Check if the author's digest available in the index entries
                T::CommitmentAdapter::entry_exists(reason, digest, author_digest)?;
                (backer, digest)
            }
            Funder::Pool { digest, backer } => {
                // Check if the author's digest available in the pool slots
                T::CommitmentAdapter::slot_exists(reason, digest, author_digest)?;
                (backer, digest)
            }
        };
        // Ensure the funder already has funded (not simply trying to draw funds)
        T::CommitmentAdapter::commit_exists(funder, reason)?;
        let exist_digest = T::CommitmentAdapter::get_commit_digest(funder, reason)?;
        // Ensure if the funder funded to the given author only
        ensure!(exist_digest == *towards, Error::<T>::FundedToAnotherDigest);
        Ok(())
    }

    /// Withdraws funds for a given author on behalf of a backer.
    ///
    /// This function allows a backer to "draw" or withdraw funds that were committed
    /// to an author. Depending on the type of backer, the withdrawal behaves slightly differently:
    ///
    /// - **Direct Backer:** Withdraws the funds directly committed by the backer.
    /// - **Index Backer:** Withdraws the total funds of the specified index, assuming the author is part of it.
    /// - **Pool Backer:** Withdraws the total funds of the pool, assuming the author is part of it.
    ///
    /// Returns the withdrawn amount on success, or a `DispatchError` if validation fails.
    fn draw(from: &Author<T>, by: &Self::Backer) -> Result<Self::Asset, DispatchError> {
        // Validate that the backer can draw funds for the author
        Self::can_draw(by, from)?;

        // Define the reason for freezing funds during the withdrawal
        let reason = &FreezeReason::AuthorFunding.into();

        // Identify the actual backer from the `Funder` enum
        let funder = match by {
            Funder::Direct(backer)
            | Funder::Index { digest: _, backer }
            | Funder::Pool { digest: _, backer } => backer,
        };

        // Resolve the commitment and return the withdrawn funds
        let actual = T::CommitmentAdapter::resolve_commit(funder, reason)?;

        // Update the backers in the authors meta-data
        // In case of index backer or a pool, all the funded authors must get reflected on their recent withdrawal
        match by {
            Funder::Direct(_) => {
                AuthorFunders::<T>::remove((from, funder));
            }
            Funder::Index { digest, backer: _ } => {
                let entries = T::CommitmentAdapter::get_entries_shares(reason, digest)?;
                for (entry, _) in entries {
                    let author = AuthorsDigest::<T>::get(&entry);
                    let author = author.ok_or(Error::<T>::AuthorDigestNotFound)?;
                    AuthorFunders::<T>::remove((author, funder));
                }
            }
            Funder::Pool { digest, backer: _ } => {
                let slots = T::CommitmentAdapter::get_slots_shares(reason, digest)?;
                for (slot, _) in slots {
                    let author = AuthorsDigest::<T>::get(&slot);
                    let author = author.ok_or(Error::<T>::AuthorDigestNotFound)?;
                    AuthorFunders::<T>::remove((author, funder));
                }
            }
        }
        Self::on_drawn(from, by, actual);
        Ok(actual)
    }

    /// Returns all backers currently funding the given author along with their real-time contributions.
    ///
    /// This excludes the author's own collateral as only external backers are returned.
    ///
    /// This function iterates over each registered funder for the author and retrieves their committed value:
    /// - **Direct:** Returns the committed amount.
    /// - **Index:** Fetches the value of the author's (entry's) digest within the index's digest's entries.
    /// - **Pool:** Fetches the value of the author's (slot's) digest within the pool's digest's slots.
    ///
    /// Returns a vector of `(Backer, Asset)` tuples or a `DispatchError` if any validation fails.
    fn backers_of(who: &Author<T>) -> Result<Vec<(Self::Backer, Self::Asset)>, DispatchError> {
        let info = Self::get_meta(who)?;
        let mut result: Vec<(Self::Backer, Self::Asset)> = Default::default();
        let reason = &FreezeReason::AuthorFunding.into();
        let iter = AuthorFunders::<T>::iter_prefix((who,));
        for (_, funder) in iter {
            match &funder {
                Funder::Direct(direct) => {
                    let value = T::CommitmentAdapter::get_commit_value(direct, reason)?;
                    result.push((funder, value));
                }
                Funder::Index { digest, backer } => {
                    // Retrieve the backers's contribution from the index digest for the author (entry)
                    let value = T::CommitmentAdapter::get_entry_value_for(
                        backer,
                        reason,
                        digest,
                        &info.digest,
                    )?;
                    result.push((funder, value));
                }
                Funder::Pool { digest, backer } => {
                    // Retrieve the backers's contribution from the pool digest for the author (slot)
                    let value = T::CommitmentAdapter::get_slot_value_for(
                        backer,
                        reason,
                        digest,
                        &info.digest,
                    )?;
                    result.push((funder, value));
                }
            }
        }

        Ok(result)
    }

    /// Returns all authors currently funded by the given backer as external funding along
    /// with their real-time contributions.
    ///
    /// Behavior varies by backer type:
    /// - **Direct:** Expected to fund a single author; retrieves the commit digest and value.
    /// - **Index:** Can fund multiple authors; retrieves all index entries as digests and
    /// values for the backing account.
    /// - **Pool:** Can fund multiple authors; retrieves all pool slots as digests and values
    /// for the backing account.
    ///
    /// After retrieving digests and values, this function resolves each digest to the
    /// corresponding author and ensures that the author exists.
    ///
    /// Returns a vector of `(Author, Asset)` tuples, or a `DispatchError` if validation fails.
    fn backed_for(by: &Self::Backer) -> Result<Vec<(Author<T>, Self::Asset)>, DispatchError> {
        let mut result: Vec<(Author<T>, Self::Asset)> = Default::default();
        let mut pre_return: Vec<(AuthorDigest<T>, Self::Asset)> = Default::default();
        let reason = &FreezeReason::AuthorFunding.into();
        match by {
            Funder::Direct(funder) => {
                // Direct commit; expected to have only a single author
                let to = T::CommitmentAdapter::get_commit_digest(funder, reason)?;
                let value = T::CommitmentAdapter::get_commit_value(funder, reason)?;
                pre_return.push((to, value))
            }
            Funder::Index { digest, backer } => {
                // Retrieve all entries (author digests and values) in the index for the backer.
                pre_return = T::CommitmentAdapter::get_entries_value_for(backer, reason, digest)?;
            }
            Funder::Pool { digest, backer } => {
                // Retrieve all slots (author digests and values) in the pool for the backer.
                pre_return = T::CommitmentAdapter::get_slots_value_for(backer, reason, digest)?;
            }
        }

        // Resolve each digest to the actual author and push into the result
        for (digest, value) in pre_return {
            let author = AuthorsDigest::<T>::get(&digest);
            let author = author.ok_or(Error::<T>::AuthorDigestNotFound)?;
            result.push((author, value))
        }

        Ok(result)
    }

    /// Returns the real-time contribution a specific backer has funded to the given author.
    ///
    /// Behavior varies by backer type:
    /// - **Direct:** Expects a single author; verifies that the digest maps to the given author.
    /// - **Index:** Returns the value of the author's entry in the index for the backer account.
    /// - **Pool:** Returns the value of the author's slot in the pool for the backer account.
    ///
    /// Returns the funded `Asset` or a `DispatchError` if validation fails.
    fn get_fund(who: &Author<T>, by: &Self::Backer) -> Result<Self::Asset, DispatchError> {
        let info = Self::get_meta(who)?;
        let reason = &FreezeReason::AuthorFunding.into();

        match by {
            Funder::Direct(direct) => {
                // Ensure the direct funder's commit digest corresponds to this author
                let digest = T::CommitmentAdapter::get_commit_digest(direct, reason)?;
                let is_author =
                    AuthorsDigest::<T>::get(digest).ok_or(Error::<T>::AuthorDigestNotFound)?;
                ensure!(is_author == *who, Error::<T>::FundedToAnotherDigest,);
                T::CommitmentAdapter::get_commit_value(direct, reason)
            }
            Funder::Index { digest, backer } => {
                // Get value of this author's entry in the index for the backer.
                T::CommitmentAdapter::get_entry_value_for(backer, reason, digest, &info.digest)
            }
            Funder::Pool { digest, backer } => {
                // Get value of this author's slot in the pool for the backer.
                T::CommitmentAdapter::get_slot_value_for(backer, reason, digest, &info.digest)
            }
        }
    }

    /// Hook invoked after a backer withdraws previously committed funds
    /// from an author, via direct, index, or pool commitments.
    ///
    /// For index or pool backers, the emitted amount represents the
    /// aggregated withdrawal applied across all associated authors.
    ///
    /// Emits any one of event if [`Config::EmitEvents`] is `true`.
    ///     - Direct Author: [`Event::AuthorDrawn`]
    ///     - Index: [`Event::IndexDrawn`]
    ///     - Pool: [`Event::PoolDrawn`]
    fn on_drawn(who: &Author<T>, by: &Self::Backer, amount: Self::Asset) {
        if T::EmitEvents::get() {
            match by {
                Funder::Direct(backer) => {
                    Self::deposit_event(Event::<T>::AuthorDrawn {
                        author: who.clone(),
                        backer: backer.clone(),
                        amount,
                    });
                }
                Funder::Index { digest, backer } => {
                    Self::deposit_event(Event::<T>::IndexDrawn {
                        index: digest.clone(),
                        backer: backer.clone(),
                        amount,
                    });
                }
                Funder::Pool { digest, backer } => {
                    Self::deposit_event(Event::<T>::PoolDrawn {
                        pool: digest.clone(),
                        backer: backer.clone(),
                        amount,
                    });
                }
            }
        }
    }

    /// Hook invoked after an author is successfully funded by a backer.
    ///
    /// For index or pool backers, the emitted amount represents the
    /// aggregated deposit distributed across all associated authors.
    ///
    /// Emits any one of event if [`Config::EmitEvents`] is `true`.
    ///     - Direct Author: [`Event::AuthorFunded`]
    ///     - Index: [`Event::IndexFunded`]
    ///     - Pool: [`Event::PoolFunded`]
    fn on_funded(who: &Author<T>, by: &Self::Backer, amount: Self::Asset) {
        if T::EmitEvents::get() {
            match by {
                Funder::Direct(backer) => {
                    Self::deposit_event(Event::<T>::AuthorFunded {
                        author: who.clone(),
                        backer: backer.clone(),
                        amount,
                    });
                }
                Funder::Index { digest, backer } => {
                    Self::deposit_event(Event::<T>::IndexFunded {
                        index: digest.clone(),
                        backer: backer.clone(),
                        amount,
                    });
                }
                Funder::Pool { digest, backer } => {
                    Self::deposit_event(Event::<T>::PoolFunded {
                        pool: digest.clone(),
                        backer: backer.clone(),
                        amount,
                    });
                }
            }
        }
    }
}

// ===============================================================================
// `````````````````````````````` COMPENSATE ROLES ```````````````````````````````
// ===============================================================================

/// Implements the [`CompensateRoles`] trait for the **Author subsystem**
///
/// Defines how authors can be rewarded/slashed along with its backers
/// within the runtime.  
impl<T: Config> CompensateRoles<Author<T>> for Pallet<T> {
    /// The penalty ratio type.
    ///
    /// Uses [`Perbill`], a fixed-point representation
    /// with 1 billion precision.
    ///
    /// ## Example
    /// - `Perbill::from_percent(5)`  -> 5% penalty
    /// - `Perbill::from_parts(500_000_000)` -> 50% penalty
    type Ratio = Perbill;

    /// Checks whether the given `Author` currently has any pending rewards.
    ///
    /// - This function **only performs a read check** - it does not mutate state.
    /// - The lookup range ensures that pending rewards are checked from the *next block onward*,
    ///   accounting for rewards that may already be queued but not yet enforced.
    ///
    /// DispatchError otherwise.
    fn has_reward(who: &Author<T>) -> DispatchResult {
        // Early return if author is invalid
        Self::role_exists(who)?;

        // Since rewards are enforced via `on_initialize`, we skip the current block
        let mut start_block = frame_system::Pallet::<T>::block_number().saturating_add(1u32.into());

        // The upper bound for reward scanning - no rewards exist beyond this block.
        let last_reward_block = RewardsUntil::<T>::get();

        // Iterate through blocks up to the last known reward block.
        while start_block <= last_reward_block {
            // If a reward entry exists for this author at this block, report success.
            if AuthorRewards::<T>::contains_key((start_block, who)) {
                return Ok(());
            }
            // Advance to the next block.
            start_block = start_block.saturating_add(1u32.into())
        }

        // No pending rewards found within the valid range.
        Err(Error::<T>::RewardNotFound.into())
    }

    /// Checks whether the given `Author` currently has any pending penalties.
    ///
    /// - This function **only performs a read check** - it does not mutate state.
    /// - The lookup range ensures that pending penalties are checked from the *next block onward*,
    ///   accounting for penalries that may already be queued but not yet enforced.
    ///
    /// DispatchError otherwise.
    fn has_penalty(who: &Author<T>) -> DispatchResult {
        // Early return if author is invalid
        Self::role_exists(who)?;

        // Since penalties are enforced via `on_initialize`, we skip the current block
        let mut start_block = frame_system::Pallet::<T>::block_number().saturating_add(1u32.into());

        // The upper bound for penalty scanning - no penalties exist beyond this block.
        let last_penalty_block = PenaltiesUntil::<T>::get();

        // Iterate through blocks up to the last known penalty block.
        while start_block <= last_penalty_block {
            if AuthorPenalties::<T>::contains_key((start_block, who)) {
                return Ok(());
            }
            start_block = start_block.saturating_add(1u32.into())
        }
        // No pending rewards found within the valid range.
        Err(Error::<T>::PenaltyNotFound.into())
    }

    /// Retrieves all **pending rewards** for a given author.
    ///
    /// Rewards are finalized over time via periodic enforcement,
    /// so the current block is **skipped** since it would have been finalized
    ///
    /// ## Returns
    /// - `Ok(Vec<(TimeStamp, Asset)>)` - a list of `(block_number, reward_value)` tuples
    ///   for each reward found.  
    /// - `Err(DispatchError)` - otherwise.
    fn get_rewards_of(
        who: &Author<T>,
    ) -> Result<Vec<(Self::TimeStamp, Self::Asset)>, DispatchError> {
        // Early return if author is invalid
        Self::role_exists(who)?;

        // Accumulator for rewards
        let mut result: Vec<(Self::TimeStamp, Self::Asset)> = Default::default();

        // Since rewards are enforced via `on_initialize`, we skip the current block
        let mut start_block = frame_system::Pallet::<T>::block_number().saturating_add(1u32.into());

        // The upper bound for reward scanning - no rewards exist beyond this block.
        let last_reward_block = RewardsUntil::<T>::get();

        // Iterate through blocks up to the last known reward block.
        while start_block <= last_reward_block {
            if let Some(value) = AuthorRewards::<T>::get((start_block, who)) {
                // Reward found hence accumulate
                result.push((start_block, value))
            }
            start_block = start_block.saturating_add(1u32.into())
        }
        Ok(result)
    }

    /// Retrieves all **pending penalities** for a given author.
    ///
    /// Penalties are finalized over time via periodic enforcement,
    /// so the current block is **skipped** since it would have been finalized
    ///
    /// ## Returns
    /// - `Ok(Vec<(TimeStamp, Ratio)>)` - a list of `(block_number, factor)` tuples
    ///   for each penalty found.  
    /// - `Err(DispatchError)` - otherwise.
    fn get_penalties_of(
        who: &Author<T>,
    ) -> Result<Vec<(Self::TimeStamp, Self::Ratio)>, DispatchError> {
        // Early return if author is invalid
        Self::role_exists(who)?;

        // Accumulator for penalties
        let mut result: Vec<(Self::TimeStamp, Self::Ratio)> = Default::default();

        // Since penalties are enforced via `on_initialize`, we skip the current block
        let mut start_block = frame_system::Pallet::<T>::block_number().saturating_add(1u32.into());

        // The upper bound for penalty scanning - no penalties exist beyond this block.
        let last_penalty_block = PenaltiesUntil::<T>::get();

        // Iterate through blocks up to the last known penalty block.
        while start_block <= last_penalty_block {
            if let Some(factor) = AuthorPenalties::<T>::get((start_block, who)) {
                result.push((start_block, factor))
            }
            start_block = start_block.saturating_add(1u32.into())
        }
        Ok(result)
    }

    /// Retrieves the current **hold amount** for the specified `Author`.
    ///
    /// - This function is **read-only** and does not modify any runtime state.
    /// - The returned hold includes all **live reserved assets** for the author:
    ///   funding, collateral, and enforced rewards/penalties.
    ///
    /// DispatchError otherwise
    fn get_hold(who: &Author<T>) -> Result<Self::Asset, DispatchError> {
        let info = Self::get_meta(who)?;

        // Freeze reason for external author fundings.
        let funding_reason = &FreezeReason::AuthorFunding.into();
        let funding = T::CommitmentAdapter::get_digest_value(funding_reason, &info.digest)?;
        // Freeze reason for author collateral.
        let collateral_reason = &FreezeReason::AuthorCollateral.into();
        let collateral = T::CommitmentAdapter::get_digest_value(collateral_reason, &info.digest)?;

        // Compute total hold; fail if overflow occurs.
        let hold = funding.checked_add(&collateral);

        debug_assert!(
            hold.is_some(),
            "exhausted the asset type's max bound value by the author {:?}
            via funding {:?} + collateral {:?}, if non-issuance asset ignore 
            this, else requires strict action",
            who,
            funding,
            collateral
        );

        let hold = hold.ok_or(Error::<T>::AuthorTotalHoldExhausted)?;

        Ok(hold)
    }

    /// Retrieves all **pending rewards** for a specific timestamp across all authors.
    ///
    /// This function performs a reverse lookup of rewards scheduled for enforcement
    /// at the given `time_stamp`.
    ///
    /// Rewards at or before the current finalized block cannot be queried, as
    /// they are already settled and no longer represent pending obligations.
    ///
    /// ## Returns
    /// - `Ok(Vec<(Author<T>, Asset)>)` - a list of `(author, reward_value)` tuples
    ///   representing pending rewards for the specified timestamp.  
    /// - `Err(DispatchError)` otherwise.
    fn get_rewards_on(
        time_stamp: Self::TimeStamp,
    ) -> Result<Vec<(Author<T>, Self::Asset)>, DispatchError> {
        // Current or previous blocks rewards are finalized, hence cannot derive
        if time_stamp <= frame_system::Pallet::<T>::block_number() {
            return Err(Error::<T>::FinalizedObligations.into());
        }

        let mut result: Vec<(Author<T>, Self::Asset)> = Default::default();
        let iter = AuthorRewards::<T>::iter_prefix((time_stamp,));
        // Iterate through all pending rewards of particular timestamp
        for (author, reward) in iter {
            // Accumulate pending rewards
            result.push((author, reward))
        }
        Ok(result)
    }

    /// Retrieves all **pending penalties** for a specific timestamp across all authors.
    ///
    /// This function performs a reverse lookup of penalties scheduled for enforcement
    /// at the given `time_stamp`.
    ///
    /// Penalties at or before the current finalized block cannot be queried, as
    /// they are already settled and no longer represent pending obligations.
    ///
    /// ## Returns
    /// - `Ok(Vec<(Author<T>, Ratio)>)` - a list of `(author, factor)` tuples
    ///   representing pending penalties for the specified timestamp.  
    /// - `Err(DispatchError)` otherwise.
    fn get_penalties_on(
        time_stamp: Self::TimeStamp,
    ) -> Result<Vec<(Author<T>, Self::Ratio)>, DispatchError> {
        // Current or previous blocks penalties are finalized, hence cannot derive
        if time_stamp <= frame_system::Pallet::<T>::block_number() {
            return Err(Error::<T>::FinalizedObligations.into());
        }
        let mut result: Vec<(Author<T>, Self::Ratio)> = Default::default();
        let iter = AuthorPenalties::<T>::iter_prefix((time_stamp,));
        // Iterate through all pending penalties of particular timestamp
        for (author, factor) in iter {
            result.push((author, factor))
        }
        Ok(result)
    }

    /// Updates the **total hold amount** of an author by proportionally redistributing
    /// the specified value across all of its components.
    ///
    /// - A hold represents an aggregated value of all **live reserved assets** for the author:
    ///   funding, collateral, and enforced rewards/penalties.
    ///
    /// This function recalculates and updates these underlying components based on the
    /// new total hold value provided.
    ///
    /// ## Returns
    /// - `Ok(())` - if the total hold was successfully recalculated and updated.  
    /// - `Err(DispatchError)` - otherwise.
    fn set_hold(
        who: &Author<T>,
        value: Self::Asset,
        precision: Precision,
        force: Fortitude,
    ) -> DispatchResult {
        let info = Self::get_meta(who)?;

        // Freeze reason for external author fundings.
        let funding_reason = &FreezeReason::AuthorFunding.into();
        let funding = T::CommitmentAdapter::get_digest_value(funding_reason, &info.digest)?;

        // Freeze reason for author collateral.
        let collateral_reason = &FreezeReason::AuthorCollateral.into();
        let collateral = T::CommitmentAdapter::get_digest_value(collateral_reason, &info.digest)?;

        // Compute total hold; fail if overflow occurs.
        let hold = funding.checked_add(&collateral);

        debug_assert!(
            hold.is_some(),
            "exhausted the asset type's max bound value by the author {:?}
            via funding {:?} + collateral {:?}, if non-issuance asset ignore 
            this, else requires strict action",
            who,
            funding,
            collateral
        );

        let hold = hold.ok_or(Error::<T>::AuthorTotalHoldExhausted)?;

        let funding_ratio = <Self::Ratio as PerThing>::from_rational(funding, hold);

        // We take ceil instead of floor since external fundings are increasable unlike collateral
        // hence it holds more accountability due to its mutable influence.
        let funding_value = funding_ratio.mul_ceil(value);
        let collateral_value = value.saturating_sub(funding_value);

        // Set both holds i.e., commitment reasons an author is subjected to.
        let qualifier = <<T::CommitmentAdapter as Commitment<Author<T>>>::Intent as Directive>::new(
            precision, force,
        );
        T::CommitmentAdapter::set_digest_value(
            funding_reason,
            &info.digest,
            funding_value,
            &qualifier.clone(),
        )?;
        T::CommitmentAdapter::set_digest_value(
            collateral_reason,
            &info.digest,
            collateral_value,
            &qualifier,
        )?;
        Self::on_set_hold(who, value);
        Ok(())
    }

    /// Applies a **penalty** to a given author, scheduled for enforcement at a future block.
    ///
    /// A penalty represents a negative adjustment to the author's hold.
    ///
    /// This function registers a proportional penalty (as a [`PerThing`] factor)
    /// against all of author's commitments.
    ///
    /// Each penalty is deferred for a specified *buffer period* to allow orderly
    /// finalization and to ensure temporal separation of distinct penalty events.
    ///
    /// Applies risk to the author's permanence before applying the penalty.
    ///
    /// Additionally tries to revoke permanence for permenant authors if possible.
    ///
    /// ## Returns
    /// - `Ok(TimeStamp)` - the block number at which the penalty is scheduled to finalize.  
    /// - `Err(DispatchError)` - otherwise.
    fn penalize(who: &Author<T>, factor: Self::Ratio) -> Result<Self::TimeStamp, DispatchError> {
        // Reject zero penalties as invalid
        if factor.is_zero() {
            return Err(Error::<T>::ZeroPenaltyFound.into());
        }

        let status = Self::get_status(who)?;

        match status {
            // Active authors risk permanence
            AuthorStatus::Active => {
                let result = Self::risk_permanence(who);
                debug_assert!(
                    result.is_ok(),
                    "author {:?} active status available but cannot risk their permanance",
                    who
                );
                result?;
                if Self::can_revoke_permanence(who).is_ok() {
                    Self::revoke_permanence(who)?;
                }
            }
            // Probation authors risk probation
            AuthorStatus::Probation => {
                let result = Self::risk_probation(who);
                debug_assert!(
                    result.is_ok(),
                    "author {:?} probation status available but cannot risk their probation",
                    who
                );
                result?
            }
            // Cannot penalize resigned authors
            AuthorStatus::Resigned => {
                return Err(Error::<T>::AuthorResigned.into());
            }
        }

        // Compute initial target block for penalty enforcement using buffer
        let mut block =
            frame_system::Pallet::<T>::block_number().saturating_add(PenaltiesBuffer::<T>::get());

        // Ensure penalty is scheduled at a unique (block, author) slot
        // Loop until an empty slot is found for the author
        loop {
            if !AuthorPenalties::<T>::contains_key((block, who)) {
                AuthorPenalties::<T>::insert((block, who), &factor);
                break;
            }
            // If slot occupied, move to the next block
            block = block.saturating_add(1u32.into());
        }

        // Update system-wide latest penalty timestamp if this penalty is further in the future
        if block > PenaltiesUntil::<T>::get() {
            PenaltiesUntil::<T>::put(block);
        }

        Self::on_penalize(who, factor, block);
        // Return the scheduled block number for this penalty
        Ok(block)
    }

    /// Removes a **pending penalty** for a given author, effectively forgiving it
    /// **at a particular timestamp**.
    ///
    /// It allows the system to revoke a scheduled penalty that has not yet been finalized,
    /// identified by the specific timestamp (`from`) at which the penalty was originally set
    /// for enforcement.
    ///
    /// - Forgiveness cannot apply to penalties at or before the finalized block height.  
    /// - Author permanence is re-secured upon successful forgiveness.  
    ///
    /// ## Returns
    /// - `Ok(Ratio)` - the penalty factor that was successfully forgiven.  
    /// - `Err(DispatchError)` - otherwise
    fn forgive(who: &Author<T>, from: Self::TimeStamp) -> Result<Self::Ratio, DispatchError> {
        let status = Self::get_status(who)?;

        // Cannot forgive penalties that are already finalized (current or past blocks)
        if from <= frame_system::Pallet::<T>::block_number() {
            return Err(Error::<T>::FinalizedObligations.into());
        }

        if status == AuthorStatus::Resigned {
            return Err(Error::<T>::AuthorResigned.into());
        }

        // Retrieve the penalty factor for the specified timestamp
        let factor = AuthorPenalties::<T>::get((from, who)).ok_or(Error::<T>::PenaltyNotFound)?;
        // Remove the penalty since it is forgiven
        AuthorPenalties::<T>::remove((from, who));
        // Secure the author's permanence after forgiveness
        let result = Self::secure_permanence(who);
        debug_assert!(
            result.is_ok(),
            "author {:?} is-available (not resigned) but cannot secure permanance",
            who
        );
        result?;

        Self::on_forgive(who, factor);
        // Return the forgiven penalty factor
        Ok(factor)
    }

    /// Schedules a **reward** for a given author at a future block.
    ///
    /// A reward represents a positive adjustment to the author's hold.
    ///
    /// Each reward is deferred for a specified *buffer period* to allow orderly
    /// finalization and to ensure temporal separation of distinct penalty events.
    ///
    /// Ensures the author's permanence is secured before applying the reward.
    ///
    /// ## Returns
    /// - `Ok(TimeStamp)` - the block number at which the reward is scheduled.  
    /// - `Err(DispatchError)` - otherwise.
    /// - `Ok(TimeStamp)` - the block number at which the reward is scheduled.  
    /// - `Err(DispatchError)` - otherwise.
    fn reward(
        who: &Author<T>,
        value: Self::Asset,
        _precision: Precision,
    ) -> Result<Self::TimeStamp, DispatchError> {
        let status = Self::get_status(who)?;

        // Only Active or Probation authors can receive rewards
        // Resigned authors cannot be rewarded
        match status {
            AuthorStatus::Active | AuthorStatus::Probation => {
                // Secure the author's permanence before rewarding
                let result = Self::secure_permanence(who);
                debug_assert!(
                    result.is_ok(),
                    "author {:?} is-available (not resigned) but cannot secure permanance",
                    who
                );
                result?;
            }
            AuthorStatus::Resigned => return Err(Error::<T>::AuthorResigned.into()),
        }

        // Compute initial target block for reward scheduling using buffer
        let mut block =
            frame_system::Pallet::<T>::block_number().saturating_add(RewardsBuffer::<T>::get());

        // Ensure reward is scheduled at a unique (block, author) slot
        // Loop until an empty slot is found for the author's digest
        loop {
            if !AuthorRewards::<T>::contains_key((block, who)) {
                AuthorRewards::<T>::insert((block, who), &value);
                break;
            }
            // If slot occupied, move to the next block
            block = block.saturating_add(1u32.into());
        }

        // Update system-wide latest reward timestamp if this reward is further in the future
        if block > RewardsUntil::<T>::get() {
            RewardsUntil::<T>::put(block);
        }

        Self::on_reward(who, value, block);
        // Return the scheduled block number for this reward
        Ok(block)
    }

    /// Removes a **pending reward** for a given author, effectively regaining it
    /// **from a particular timestamp** scheduled.
    ///
    /// It allows the system to revoke a scheduled reward that has not yet been finalized,
    /// identified by the specific timestamp (`from`) at which the reward was originally set
    /// for enforcement.
    ///
    /// Regaining cannot apply to rewards at or before the finalized block height.  
    ///
    /// ## Returns
    /// - `Ok(Asset)` - the total reward that was successfully regained.  
    /// - `Err(DispatchError)` - otherwise
    fn reclaim(who: &Author<T>, from: Self::TimeStamp) -> Result<Self::Asset, DispatchError> {
        Self::role_exists(who)?;

        // Cannot reclaim rewards that are already finalized (current or past blocks)
        if from <= frame_system::Pallet::<T>::block_number() {
            return Err(Error::<T>::FinalizedObligations.into());
        }

        // Retrieve the reward value for the specified timestamp
        let value = AuthorRewards::<T>::get((from, who)).ok_or(Error::<T>::RewardNotFound)?;
        // Remove the reward since it is being reclaimed
        AuthorRewards::<T>::remove((from, who));

        Self::on_reclaim(who, value);
        // Return the reclaimed reward value
        Ok(value)
    }

    /// Hook invoked when reward is scheduled or applied to an author.
    ///
    /// Emits [`Event::AuthorRewardScheduled`] if [`Config::EmitEvents`] is `true`.
    fn on_reward(who: &Author<T>, amount: Self::Asset, at: Self::TimeStamp) {
        if T::EmitEvents::get() {
            Self::deposit_event(Event::<T>::AuthorRewardScheduled {
                author: who.clone(),
                amount: amount,
                at: at,
            });
        }
    }

    /// Hook invoked when an author's scheduled rewards are reclaimed.
    ///
    /// Emits [`Event::AuthorRewardReclaimed`] if [`Config::EmitEvents`] is `true`.
    fn on_reclaim(who: &Author<T>, amount: Self::Asset) {
        if T::EmitEvents::get() {
            Self::deposit_event(Event::<T>::AuthorRewardReclaimed {
                author: who.clone(),
                amount,
            });
        }
    }

    /// Hook invoked when an author's hold balance is updated.
    ///
    /// Emits [`Event::AuthorTotalHold`] if [`Config::EmitEvents`] is `true`.
    fn on_set_hold(who: &Author<T>, value: Self::Asset) {
        if T::EmitEvents::get() {
            Self::deposit_event(Event::<T>::AuthorTotalHold {
                author: who.clone(),
                value: value,
            });
        }
    }

    /// Hook invoked when an author's scheduled penalty is forgiven.
    ///
    /// Emits [`Event::AuthorPenaltyForgiven`] if [`Config::EmitEvents`] is `true`.
    fn on_forgive(who: &Author<T>, factor: Self::Ratio) {
        if T::EmitEvents::get() {
            Self::deposit_event(Event::<T>::AuthorPenaltyForgiven {
                author: who.clone(),
                factor,
            });
        }
    }

    /// Hook invoked when penality is scheduled or applied to an author.
    ///
    /// Emits [`Event::AuthorPenaltyScheduled`] if [`Config::EmitEvents`] is `true`.
    fn on_penalize(who: &Author<T>, factor: Self::Ratio, at: Self::TimeStamp) {
        if T::EmitEvents::get() {
            Self::deposit_event(Event::<T>::AuthorPenaltyScheduled {
                author: who.clone(),
                factor,
                at: at,
            });
        }
    }
}

// ===============================================================================
// ``````````````````````````````` ROLE PROBATION ````````````````````````````````
// ===============================================================================

/// Implements the [`RoleProbation`] trait for the **Author subsystem**
///
/// Defines how authors can be switched between probation and permenance
/// with certain invariants enforced for good behavior.
impl<T: Config> RoleProbation<Author<T>> for Pallet<T> {
    /// Checks if the author is currently under probation.
    ///
    /// Returns `Ok(())` if the author is in `Probation` status.
    /// DispatchError of current status of author otherwise.
    fn is_on_probation(who: &Author<T>) -> DispatchResult {
        let status = Self::get_status(who)?;
        match status {
            AuthorStatus::Active => Err(Error::<T>::AuthorIsActive.into()),
            AuthorStatus::Probation => Ok(()),
            AuthorStatus::Resigned => Err(Error::<T>::AuthorResigned.into()),
        }
    }

    /// Checks if the author has secured permanent (active) status.
    ///
    /// Returns `Ok(())` if the author is `Active` (permanent).
    /// DispatchError of current status of author otherwise.
    fn is_permanent(who: &Author<T>) -> DispatchResult {
        let status = Self::get_status(who)?;
        match status {
            AuthorStatus::Active => Ok(()),
            AuthorStatus::Probation => Err(Error::<T>::AuthorInProbation.into()),
            AuthorStatus::Resigned => Err(Error::<T>::AuthorResigned.into()),
        }
    }

    /// Checks if the given author is eligible to become permanent.
    ///
    /// Evaluates risk status and requires author in probation.
    ///
    /// - Returns `Ok(())` if the author can be promoted to permanent status.
    /// - Returns `Err(DispatchError)` otherwise.
    fn can_be_permanent(who: &Author<T>) -> DispatchResult {
        let info = Self::get_meta(who)?;
        let status = &info.status;

        // Only authors in Probation can be evaluated for permanence
        match status {
            // Active authors cannot be made permanent
            AuthorStatus::Active => return Err(Error::<T>::AuthorIsActive.into()),
            // Resigned authors cannot be made permanent
            AuthorStatus::Resigned => return Err(Error::<T>::AuthorResigned.into()),
            // Probation authors are eligible for further checks
            AuthorStatus::Probation => {}
        }

        let current_block = frame_system::Pallet::<T>::block_number();
        let status_since = info.status_since;

        // Check if the probation period has elapsed
        if status_since.saturating_add(ProbationPeriod::<T>::get()) > current_block {
            // Author is still within probation period
            return Err(Error::<T>::AuthorInProbation.into());
        }

        // Check if the author is currently under risk evaluation
        let risk_until = info.risk_until;
        if risk_until > current_block {
            return Err(Error::<T>::AuthorIsUnsafe.into());
        }
        Ok(())
    }

    /// Promotes an author to permanent/active status.
    ///
    /// Returns `Ok(AuthorStatus::Active)` on success or `Err(DispatchError)` otherwise.
    fn set_permanence(who: &Author<T>) -> Result<Self::Status, DispatchError> {
        // Ensure the author is eligible to become permanent
        Self::can_be_permanent(who)?;

        let active = AuthorStatus::Active;

        AuthorsMap::<T>::mutate(who, |author| -> DispatchResult {
            let info = author.as_mut();
            debug_assert!(
                info.is_some(),
                "author {:?} can-be-permanent but cannot mutate status",
                who
            );
            let info = info.ok_or(Error::<T>::AuthorNotFound)?;
            let status = &mut info.status;

            // Set author status to Active
            *status = active.clone();

            Ok(())
        })?;
        Self::on_set_permance(who);
        Ok(active)
    }

    /// Checks if the given author is eligible to be placed back under probation.
    ///
    /// Passes if indication of significant risk on active authors.
    ///
    /// - Returns `Ok(())` if the author can be probated.
    /// - Returns `Err(DispatchError)` if the author is already in probation, resigned, or cannot be probated.
    fn can_revoke_permanence(who: &Author<T>) -> DispatchResult {
        let meta = Self::get_meta(who)?;
        let status = meta.status;
        let risk_until = meta.risk_until;
        let current_block = frame_system::Pallet::<T>::block_number();

        // Only Active authors permanence can be revoked
        match status {
            // Already in probation
            AuthorStatus::Probation => return Err(Error::<T>::AuthorInProbation.into()),
            // Resigned authors cannot be probated
            AuthorStatus::Resigned => return Err(Error::<T>::AuthorResigned.into()),
            // Active authors permanence may be revoked
            AuthorStatus::Active => {}
        }

        if risk_until <= current_block.saturating_add(ProbationPeriod::<T>::get()) {
            return Err(Error::<T>::RiskWithinThreshold.into());
        }
        Ok(())
    }

    /// Revokes an author's permanent/active status and places them back under probation.
    ///
    /// Returns `Ok(AuthorStatus::Probation)` on success or `Err(DispatchError)` otherwise.
    fn revoke_permanence(who: &Author<T>) -> Result<Self::Status, DispatchError> {
        // Ensure the author is eligible to be moved back to probation
        Self::can_revoke_permanence(who)?;

        let probation = AuthorStatus::Probation;
        let current_block = frame_system::Pallet::<T>::block_number();

        AuthorsMap::<T>::mutate(who, |author| -> DispatchResult {
            let info = author.as_mut();
            debug_assert!(
                info.is_some(),
                "author {:?} can-revoke-permanence but cannot mutate status",
                who
            );
            let info = info.ok_or(Error::<T>::AuthorNotFound)?;
            let status = &mut info.status;
            let status_since = &mut info.status_since;
            // Set author status to Probation
            *status = probation.clone();
            // Update timestamp of status update
            *status_since = current_block;

            Ok(())
        })?;
        Self::on_revoke_permanence(who);
        Ok(probation)
    }

    /// Marks a probationary author as at risk, extending their risk magnitude.
    ///
    /// - Returns `Ok(())` on success or `Err(DispatchError)` otherwise.
    fn risk_probation(who: &Author<T>) -> DispatchResult {
        AuthorsMap::<T>::mutate(who, |author| -> DispatchResult {
            let info = author.as_mut().ok_or(Error::<T>::AuthorNotFound)?;

            let status = &mut info.status;

            // Only authors currently in Probation can be placed at risk
            match status {
                // Active authors cannot be risked for probation
                AuthorStatus::Active => return Err(Error::<T>::AuthorIsActive.into()),
                // Resigned authors cannot be risked
                AuthorStatus::Resigned => return Err(Error::<T>::AuthorResigned.into()),
                // Probation authors are eligible
                AuthorStatus::Probation => {}
            }

            let current_block = frame_system::Pallet::<T>::block_number();
            let risk_until = &mut info.risk_until;

            // If risk has expired, reset from current block otherwise extend from existing risk_until
            if *risk_until < current_block {
                *risk_until = current_block.saturating_add(IncreaseProbationBy::<T>::get());
                return Ok(());
            }
            *risk_until = risk_until.saturating_add(IncreaseProbationBy::<T>::get());

            Ok(())
        })?;
        Self::on_risk_probation(who);
        Ok(())
    }

    /// Marks a permanent/active author as at risk, potentially impacting their permanence.
    ///
    /// - Returns `Ok(())` on success or `Err(DispatchError)` otherwise.
    fn risk_permanence(who: &Author<T>) -> DispatchResult {
        AuthorsMap::<T>::mutate(who, |author| -> DispatchResult {
            // Fetch author metadata; fail early if not found
            let info = author.as_mut().ok_or(Error::<T>::AuthorNotFound)?;

            let status = &mut info.status;

            // Only Active authors can have their permanence risked
            match status {
                // Active authors are eligible
                AuthorStatus::Active => {}
                // Resigned authors cannot be risked
                AuthorStatus::Resigned => return Err(Error::<T>::AuthorResigned.into()),
                // Probation authors cannot be risked here
                AuthorStatus::Probation => return Err(Error::<T>::AuthorInProbation.into()),
            }

            let current_block = frame_system::Pallet::<T>::block_number();
            let risk_until = &mut info.risk_until;

            // If risk has expired, reset from current block otherwise extend from existing risk_until
            if *risk_until < current_block {
                *risk_until = current_block.saturating_add(IncreaseProbationBy::<T>::get());
                return Ok(());
            }
            *risk_until = risk_until.saturating_add(IncreaseProbationBy::<T>::get());
            Ok(())
        })?;
        Self::on_risk_permanence(who);
        Ok(())
    }

    /// Reduces the risk period for an author, securing their permanence.
    ///
    /// - Returns `Ok(())` on success or `Err(DispatchError)` otherwise.
    fn secure_permanence(who: &Author<T>) -> DispatchResult {
        AuthorsMap::<T>::mutate(who, |author| -> DispatchResult {
            let info = author.as_mut().ok_or(Error::<T>::AuthorNotFound)?;

            let status = &mut info.status;

            // Only Active or Probation authors can have their permanence secured
            match status {
                // Active authors can reduce risking to probation
                AuthorStatus::Active => {}
                // Resigned authors cannot be modified
                AuthorStatus::Resigned => return Err(Error::<T>::AuthorResigned.into()),
                // Probation authors can reduce risking negative performance
                AuthorStatus::Probation => {}
            }
            let risk_until = &mut info.risk_until;
            let current_block = frame_system::Pallet::<T>::block_number();

            // Only reduce risk if it has considerable magnitude, else do nothing
            if *risk_until > current_block {
                *risk_until = risk_until.saturating_sub(ReduceProbationBy::<T>::get());
                return Ok(());
            }
            Ok(())
        })?;
        Self::on_secure_permanence(who);
        Ok(())
    }

    /// Hook invoked after an author is promoted to permanent (Active) status.
    ///
    /// Emits [`Event::AuthorStatus`] if [`Config::EmitEvents`] is `true`.
    fn on_set_permance(who: &Author<T>) {
        if T::EmitEvents::get() {
            Self::deposit_event(Event::AuthorStatus {
                author: who.clone(),
                status: AuthorStatus::Active,
            });
        }
    }

    /// Hook invoked after an author's permanence is revoked.
    ///
    /// Emits [`Event::AuthorStatus`] if [`Config::EmitEvents`] is `true`.
    fn on_revoke_permanence(who: &Author<T>) {
        if T::EmitEvents::get() {
            Self::deposit_event(Event::AuthorStatus {
                author: who.clone(),
                status: AuthorStatus::Probation,
            });
        }
    }

    /// Hook invoked when risk is applied to an author increasing their
    /// risk towards disinheriting permanace.
    ///
    /// Emits [`Event::AuthorAtRisk`] if [`Config::EmitEvents`] is `true`.
    fn on_risk_permanence(who: &Author<T>) {
        if T::EmitEvents::get() {
            let Ok(meta) = Self::get_meta(who) else {
                return;
            };
            Self::deposit_event(Event::<T>::AuthorAtRisk {
                author: who.clone(),
                status: AuthorStatus::Active,
                until: meta.risk_until,
            });
        }
    }

    /// Hook invoked when risk is applied to an author increasing their
    /// risk to inherit permanace.
    ///
    /// Emits [`Event::AuthorAtRisk`] if [`Config::EmitEvents`] is `true`.
    fn on_risk_probation(who: &Author<T>) {
        if T::EmitEvents::get() {
            let Ok(meta) = Self::get_meta(who) else {
                return;
            };
            Self::deposit_event(Event::<T>::AuthorAtRisk {
                author: who.clone(),
                status: AuthorStatus::Probation,
                until: meta.risk_until,
            });
        }
    }

    /// Hook invoked when risk is reduced to an author increasing their
    /// oppurtunity to inherit permanace.
    ///
    /// Emits [`Event::AuthorAtRisk`] if [`Config::EmitEvents`] is `true`.
    fn on_secure_permanence(who: &Author<T>) {
        if T::EmitEvents::get() {
            let Ok(meta) = Self::get_meta(who) else {
                return;
            };
            Self::deposit_event(Event::<T>::AuthorAtRisk {
                author: who.clone(),
                status: AuthorStatus::Probation,
                until: meta.risk_until,
            });
        }
    }
}

// ===============================================================================
// `````````````````````````````````` UNIT TESTS `````````````````````````````````
// ===============================================================================

#[cfg(test)]
mod tests {
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` IMPORTS ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    // --- Local crate imports ---
    use crate::mock::*;
    use crate::{
        types::{AuthorStatus, Funder},
        Event::*,
    };

    // --- FRAME Suite ---
    use frame_suite::{commitment::*, roles::*};

    // --- FRAME Support ---
    use frame_support::{
        assert_err, assert_ok,
        traits::{
            fungible::InspectFreeze,
            tokens::{Fortitude, Precision},
        },
    };

    // --- Substrate primitives ---
    use sp_runtime::{DispatchError, Perbill};

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````` ROLE MANAGER `````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn enroll_success() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            assert_err!(Pallet::role_exists(&ALICE), Error::AuthorNotFound);
            // enroll author
            let collateral_locked = Pallet::enroll(&ALICE, LARGE_VALUE, Fortitude::Force).unwrap();
            assert_ok!(CommitAdapter::commit_exists(&ALICE, &COLLATERAL.into()));
            assert_eq!(
                AuthorAsset::balance_frozen(&COLLATERAL.into(), &ALICE),
                collateral_locked
            );
            // author enrolled
            assert_ok!(Pallet::role_exists(&ALICE));
            let author_digest = gen_author_digest(&ALICE).unwrap();
            assert_eq!(AuthorsDigest::get(author_digest), Some(ALICE));
            System::assert_last_event(Event::AuthorEnlisted { author: ALICE, collateral: collateral_locked }.into());
        })
    }

    #[test]
    fn role_exists_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            Pallet::enroll(&ALICE, LARGE_VALUE, Fortitude::Force).unwrap();
            assert_ok!(Pallet::role_exists(&ALICE));
        })
    }

    #[test]
    fn role_exists_err_author_not_found() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            Pallet::enroll(&ALICE, LARGE_VALUE, Fortitude::Force).unwrap();
            // BOB is not enrolled
            assert_err!(Pallet::role_exists(&BOB), Error::AuthorNotFound);
        })
    }

    #[test]
    fn get_meta_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, LARGE_VALUE, Fortitude::Force).unwrap();
            let author_digest = gen_author_digest(&ALICE).unwrap();
            let meta = Pallet::get_meta(&ALICE).unwrap();
            assert_eq!(meta.digest, author_digest);
            assert_eq!(meta.since, 6);
            assert_eq!(meta.status, AuthorStatus::Probation);
            assert_eq!(meta.status_since, 6);
            assert_eq!(meta.risk_until, 6);
            assert_eq!(meta.max_fund, None);
            assert_eq!(meta.min_fund, None);
        })
    }

    #[test]
    fn get_meta_err_author_not_found() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, LARGE_VALUE, Fortitude::Force).unwrap();
            // BOB is not enrolled
            assert_err!(Pallet::get_meta(&BOB), Error::AuthorNotFound);
        })
    }

    #[test]
    fn can_enroll_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            assert_ok!(Pallet::can_enroll(&ALICE, LARGE_VALUE));
        })
    }

    #[test]
    fn can_enroll_err_already_enrolled() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, LARGE_VALUE, Fortitude::Force).unwrap();
            assert_err!(
                Pallet::can_enroll(&ALICE, LARGE_VALUE),
                Error::AlreadyEnrolled
            );
        })
    }

    #[test]
    fn can_enroll_err_inadequate_collateral() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            assert_err!(
                Pallet::can_enroll(&ALICE, SMALL_VALUE),
                Error::InadequateCollateral
            );
        })
    }

    #[test]
    fn can_enroll_err_inadequate_funds() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, MIN_VALUE, MIN_VALUE).unwrap();
            assert_err!(
                Pallet::can_enroll(&ALICE, LARGE_VALUE),
                Error::InadequateFunds
            );
        })
    }

    #[should_panic]
    #[test]
    fn can_enroll_panic_author_resigned_with_penalty() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(10);
            Pallet::penalize(&ALICE, Perbill::from_percent(5)).unwrap();

            AuthorsMap::mutate(ALICE, |author| {
                let info = author.as_mut().unwrap();
                let status = &mut info.status;
                *status = AuthorStatus::Resigned
            });

            // should, panic
            Pallet::can_enroll(&ALICE, STANDARD_VALUE).unwrap();
        })
    }

    #[test]
    fn can_enroll_err_author_has_rewards() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(10);
            Pallet::reward(&ALICE, 5, Precision::BestEffort).unwrap();

            AuthorsMap::mutate(ALICE, |author| {
                let info = author.as_mut().unwrap();
                let status = &mut info.status;
                *status = AuthorStatus::Resigned
            });

            // since, rewards are scheduled
            assert_err!(
                Pallet::can_enroll(&ALICE, STANDARD_VALUE),
                Error::AuthorHasRewards
            );
        })
    }

    #[test]
    fn can_resign_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, LARGE_VALUE, Fortitude::Force).unwrap();

            AuthorsMap::mutate(ALICE, |author| {
                let info = author.as_mut().unwrap();
                let status = &mut info.status;
                *status = AuthorStatus::Active
            });

            // Set author as in-active
            set_activity_state(false);

            assert_ok! {
                Pallet::can_resign(&ALICE)
            };
        })
    }

    #[test]
    fn can_resign_err_author_in_probation() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, LARGE_VALUE, Fortitude::Force).unwrap();

            assert_err! {
                Pallet::can_resign(
                &ALICE,
            ), Error::AuthorInProbation
            };
        })
    }

    #[test]
    fn can_resign_err_redundant_resignation() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, LARGE_VALUE, Fortitude::Force).unwrap();

            AuthorsMap::mutate(ALICE, |author| {
                let info = author.as_mut().unwrap();
                let status = &mut info.status;
                *status = AuthorStatus::Resigned
            });

            assert_err! {
                Pallet::can_resign(&ALICE),
                Error::RedundantResignation
            };
        })
    }

    #[test]
    fn can_resign_err_author_has_penalties() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(10);
            Pallet::penalize(&ALICE, Perbill::from_percent(5)).unwrap();

            AuthorsMap::mutate(ALICE, |author| {
                let info = author.as_mut().unwrap();
                let status = &mut info.status;
                *status = AuthorStatus::Active
            });

            set_activity_state(false);
            // since, penalties are scheduled
            assert_err!(Pallet::can_resign(&ALICE), Error::AuthorHasPenalties);
        })
    }

    #[test]
    fn can_resign_err_author_is_active() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, LARGE_VALUE, Fortitude::Force).unwrap();

            AuthorsMap::mutate(ALICE, |author| {
                let info = author.as_mut().unwrap();
                let status = &mut info.status;
                *status = AuthorStatus::Active
            });

            // Set author as active
            set_activity_state(true);

            assert_err! {
                Pallet::can_resign(&ALICE),
                DispatchError::Other("AuthorIsActive")
            };
        })
    }

    #[test]
    fn get_collateral_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, LARGE_VALUE, Fortitude::Force).unwrap();

            let actual_collateral = Pallet::get_collateral(&ALICE).unwrap();

            assert_eq!(actual_collateral, LARGE_VALUE);
        })
    }

    #[test]
    fn total_collateral_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, LARGE_VALUE, Fortitude::Force).unwrap();

            let total_collateral = Pallet::total_collateral();
            assert_eq!(total_collateral, 100); // collateral of ALICE

            System::set_block_number(10);
            Pallet::enroll(&BOB, STANDARD_VALUE, Fortitude::Force).unwrap();

            let total_collateral = Pallet::total_collateral();
            assert_eq!(total_collateral, 150); // collateral of ALICE + BOB
        })
    }

    #[test]
    fn enroll_since_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, LARGE_VALUE, Fortitude::Force).unwrap();

            assert_eq!(Pallet::enroll_since(&ALICE), Ok(6));
        })
    }

    #[test]
    fn get_status_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, LARGE_VALUE, Fortitude::Force).unwrap();

            assert_eq!(Pallet::get_status(&ALICE), Ok(AuthorStatus::Probation));
        })
    }

    #[test]
    fn status_since_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, LARGE_VALUE, Fortitude::Force).unwrap();

            assert_eq!(Pallet::status_since(&ALICE), Ok(6));
        })
    }

    #[test]
    fn set_status_success_from_probation() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, LARGE_VALUE, Fortitude::Force).unwrap();

            let current_status = Pallet::get_status(&ALICE).unwrap();

            assert_eq!(current_status, AuthorStatus::Probation);

            // updating the status of author from AuthorStatus::Probation -> AuthorStatus::Probation is no-op
            assert_ok! {
                    Pallet::set_status(
                    &ALICE,
                    AuthorStatus::Probation
            )};

            // updating the status of author from AuthorStatus::Probation -> AuthorStatus::Resigned
            // will cause error as the author still under probation period
            assert_err! {
                Pallet::set_status(
                    &ALICE,
                    AuthorStatus::Resigned
                ),
                Error::AuthorInProbation
            };

            System::set_block_number(14);
            // updating the status of author from AuthorStatus::Probation -> AuthorStatus::Active
            // will cause error as block number still not exceeds the probation period
            assert_err! {
                Pallet::set_status(
                &ALICE,
                AuthorStatus::Active
            ), Error::AuthorInProbation
            };

            System::set_block_number(20);
            // update the status of author: AuthorStatus::Probation -> AuthorStatus::Active
            assert_ok!(Pallet::set_status(&ALICE, AuthorStatus::Active));

            let current_status = Pallet::get_status(&ALICE).unwrap();

            assert_eq!(current_status, AuthorStatus::Active);

            System::assert_last_event(Event::AuthorStatus {
                 author: ALICE, 
                 status: AuthorStatus::Active 
                }
                .into()
            );
        })
    }

    #[test]
    fn set_status_success_from_active() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, LARGE_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(20);
            // update the status of author: AuthorStatus::Probation -> AuthorStatus::Active
            assert_ok!(Pallet::set_status(&ALICE, AuthorStatus::Active));

            let current_status = Pallet::get_status(&ALICE).unwrap();
            assert_eq!(current_status, AuthorStatus::Active);

            AuthorsMap::mutate(ALICE, |author| {
                let info = author.as_mut().unwrap();
                let risk = &mut info.risk_until;
                *risk = 32
            });
            // since the risk_until is high, update the status of author
            // AuthorStatus::Active -> AuthorStatus::Probation
            assert_ok!(Pallet::set_status(&ALICE, AuthorStatus::Probation));

            System::set_block_number(42);
            // after the probation period, update the status of author
            // AuthorStatus::Probation -> AuthorStatus::Active
            assert_ok!(Pallet::set_status(&ALICE, AuthorStatus::Active));
            let current_status = Pallet::get_status(&ALICE).unwrap();
            assert_eq!(current_status, AuthorStatus::Active);

            System::set_block_number(45);
            // update the status of author
            // AuthorStatus::Active -> AuthorStatus::Resign
            assert_ok!(Pallet::set_status(&ALICE, AuthorStatus::Resigned));
            let current_status = Pallet::get_status(&ALICE).unwrap();
            assert_eq!(current_status, AuthorStatus::Resigned);
        })
    }

    #[test]
    fn set_status_success_from_resigned() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, LARGE_VALUE, Fortitude::Force).unwrap();

            AuthorsMap::mutate(ALICE, |author| {
                let info = author.as_mut().unwrap();
                let status = &mut info.status;
                *status = AuthorStatus::Resigned
            });

            let current_status = Pallet::get_status(&ALICE).unwrap();

            assert_eq!(current_status, AuthorStatus::Resigned);

            // updating the status of author from AuthorStatus::Resigned -> AuthorStatus::Probation
            // will cause error as resigned authors can be reactivated only through enrollment
            assert_err! {
                Pallet::set_status(
                &ALICE,
                AuthorStatus::Probation
            ), Error::AuthorResigned
            };

            // updating the status of author from AuthorStatus::Resigned -> AuthorStatus::Active
            // also will cause error as resigned authors can be reactivated only through enrollment
            assert_err! {
                Pallet::set_status(
                &ALICE,
                AuthorStatus::Active
            ), Error::AuthorResigned
            };

            // updating the status of author from AuthorStatus::Resigned -> AuthorStatus::Active is no-op
            assert_ok! {
                    Pallet::set_status(
                    &ALICE,
                    AuthorStatus::Resigned
            )};
        })
    }

    #[test]
    fn resign_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, LARGE_VALUE, Fortitude::Force).unwrap();

            AuthorsMap::mutate(ALICE, |author| {
                let info = author.as_mut().unwrap();
                let status = &mut info.status;
                *status = AuthorStatus::Active
            });

            let current_collateral = Pallet::get_collateral(&ALICE).unwrap();
            assert_eq!(current_collateral, LARGE_VALUE);

            let author_bal = get_user_balance(&ALICE);
            assert_eq!(author_bal, 100);

            // Set author as in-active
            set_activity_state(false);
            assert_ok!(Pallet::resign(&ALICE));

            assert_eq!(Pallet::get_status(&ALICE), Ok(AuthorStatus::Resigned));

            let author_bal = get_user_balance(&ALICE);
            assert_eq!(author_bal, 200); // existing balance + collateral
            System::assert_last_event(Event::AuthorResigned { author: ALICE, released: 100 }.into());
        })
    }

    #[test]
    fn add_collateral_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();
            let current_collateral = Pallet::get_collateral(&ALICE).unwrap();
            assert_eq!(current_collateral, 50);

            assert_ok!(Pallet::add_collateral(
                &ALICE,
                STANDARD_VALUE,
                Fortitude::Force
            ));
            // collateral increaced by 50
            let current_collateral = Pallet::get_collateral(&ALICE).unwrap();
            assert_eq!(current_collateral, 100);
            System::assert_last_event(Event::AuthorCollateralRaised { author: ALICE, raised: STANDARD_VALUE }.into());
        })
    }

    #[test]
    fn is_available_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            assert_ok!(Pallet::is_available(&ALICE));
        })
    }

    #[test]
    fn is_available_err_author_needs_more_collateral() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();
            Pallet::fund(
                &ALICE,
                &Funder::Direct(BOB),
                25,
                Precision::BestEffort,
                Fortitude::Force,
            )
            .unwrap();

            Pallet::set_hold(&ALICE, 15, Precision::Exact, Fortitude::Force).unwrap();

            assert_err!(
                Pallet::is_available(&ALICE),
                Error::AuthorNeedsMoreCollateral
            );
        })
    }

    #[test]
    fn is_available_err_author_resigned() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            AuthorsMap::mutate(ALICE, |author| {
                let info = author.as_mut().unwrap();
                let status = &mut info.status;
                *status = AuthorStatus::Resigned
            });

            assert_err!(Pallet::is_available(&ALICE), Error::AuthorResigned);
        })
    }

    #[test]
    fn on_enroll_emit_event_success() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            let collateral = LARGE_VALUE;
            Pallet::on_enroll(&ALICE, collateral);

            System::assert_last_event(
                Event::AuthorEnlisted {
                    author: ALICE,
                    collateral,
                }
                .into(),
            );
        })
    }

    #[test]
    fn on_resign_emit_event_suucess() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            let released = LARGE_VALUE;
            Pallet::on_resign(&ALICE, released);

            System::assert_last_event(
                Event::AuthorResigned {
                    author: ALICE,
                    released,
                }
                .into(),
            );
        })
    }

    #[test]
    fn on_add_collateral_emit_event_suucess() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            let raised = LARGE_VALUE;
            Pallet::on_add_collateral(&ALICE, raised);

            System::assert_last_event(
                Event::AuthorCollateralRaised {
                    author: ALICE,
                    raised,
                }
                .into(),
            );
        })
    }

    #[test]
    fn on_status_update_emit_event_suucess() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            let status = AuthorStatus::Active;
            Pallet::on_status_update(&ALICE, &status);

            System::assert_last_event(
                Event::AuthorStatus {
                    author: ALICE,
                    status,
                }
                .into(),
            );
        })
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ````````````````````````````````` FUND ROLES ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn has_funds_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            Pallet::fund(
                &ALICE,
                &Funder::Direct(BOB),
                STANDARD_VALUE,
                Precision::BestEffort,
                Fortitude::Force,
            )
            .unwrap();

            assert_ok!(Pallet::has_funds(&ALICE));
        })
    }

    #[test]
    fn has_funds_err_fund_does_not_exist() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            assert_err!(Pallet::has_funds(&ALICE), Error::FundDoesNotExist);
        })
    }

    #[test]
    fn can_fund_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            assert_ok!(Pallet::can_fund(
                &Funder::Direct(BOB),
                &ALICE,
                STANDARD_VALUE,
                Precision::Exact,
                Fortitude::Force
            ));
        })
    }

    #[test]
    fn can_fund_err_below_minimum_fund() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            assert_err!(
                Pallet::can_fund(
                    &Funder::Direct(BOB),
                    &ALICE,
                    MIN_VALUE,
                    Precision::Exact,
                    Fortitude::Force
                ),
                Error::BelowMinimumFund
            );
        })
    }

    #[test]
    fn can_fund_err_above_maximum_exposure() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            assert_err!(
                Pallet::can_fund(
                    &Funder::Direct(BOB),
                    &ALICE,
                    1100, // 1100 which is higher than the
                    // max_exposure which is set to be 1000 in mock.rs
                    Precision::Exact,
                    Fortitude::Force
                ),
                Error::AboveMaximumExposure
            );
        })
    }

    #[test]
    fn can_fund_err_fund_to_another_digest() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();
            Pallet::enroll(&BOB, STANDARD_VALUE, Fortitude::Force).unwrap();
            Pallet::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                SMALL_VALUE,
                Precision::BestEffort,
                Fortitude::Force,
            )
            .unwrap();

            assert_err!(
                Pallet::can_fund(
                    &Funder::Direct(CHARLIE),
                    &BOB,
                    LARGE_VALUE,
                    Precision::Exact,
                    Fortitude::Force
                ),
                Error::FundedToAnotherDigest
            );
        })
    }

    #[test]
    fn can_draw_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            Pallet::fund(
                &ALICE,
                &Funder::Direct(BOB),
                SMALL_VALUE,
                Precision::BestEffort,
                Fortitude::Force,
            )
            .unwrap();

            assert_ok!(Pallet::can_draw(&Funder::Direct(BOB), &ALICE,));
        })
    }

    #[test]
    fn can_draw_err_fund_to_another_digest() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();
            Pallet::enroll(&BOB, STANDARD_VALUE, Fortitude::Force).unwrap();
            Pallet::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                SMALL_VALUE,
                Precision::BestEffort,
                Fortitude::Force,
            )
            .unwrap();

            assert_err!(
                Pallet::can_draw(&Funder::Direct(CHARLIE), &BOB),
                Error::FundedToAnotherDigest
            );
        })
    }

    #[test]
    fn max_exposure_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            let max_exposure = Pallet::max_exposure(
                &Funder::Direct(BOB),
                &ALICE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();
            assert_eq!(MaxExposure::get(), max_exposure);
        })
    }

    #[test]
    fn min_fund_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            let min_fund = Pallet::min_fund(
                &Funder::Direct(BOB),
                &ALICE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();
            assert_eq!(MinFund::get(), min_fund);
        })
    }

    #[test]
    fn backed_value_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            // BOB backed ALICE with 50 units
            Pallet::fund(
                &ALICE,
                &Funder::Direct(BOB),
                STANDARD_VALUE,
                Precision::BestEffort,
                Fortitude::Force,
            )
            .unwrap();

            let current_backed_val = Pallet::backed_value(&ALICE).unwrap();
            assert_eq!(current_backed_val, 50);

            // CHARLIE backed ALICE with 25 units
            Pallet::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                SMALL_VALUE,
                Precision::BestEffort,
                Fortitude::Force,
            )
            .unwrap();

            let current_backed_val = Pallet::backed_value(&ALICE).unwrap();
            assert_eq!(current_backed_val, 75); // BOB + CHARLIE = 50 + 25 -> 75
        })
    }

    #[test]
    fn total_backing_sucess() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(10);
            Pallet::enroll(&MIKE, STANDARD_VALUE, Fortitude::Force).unwrap();

            // BOB backed ALICE with 50 units
            Pallet::fund(
                &ALICE,
                &Funder::Direct(BOB),
                STANDARD_VALUE,
                Precision::BestEffort,
                Fortitude::Force,
            )
            .unwrap();

            let current_total_backed_val = Pallet::total_backing();
            assert_eq!(current_total_backed_val, 50);

            // CHARLIE backed MIKE with 100 units
            Pallet::fund(
                &MIKE,
                &Funder::Direct(CHARLIE),
                LARGE_VALUE,
                Precision::BestEffort,
                Fortitude::Force,
            )
            .unwrap();

            // BOB increase the backing by 25 units
            Pallet::fund(
                &ALICE,
                &Funder::Direct(BOB),
                SMALL_VALUE,
                Precision::BestEffort,
                Fortitude::Force,
            )
            .unwrap();

            let current_total_backed_val = Pallet::total_backing();
            assert_eq!(current_total_backed_val, 175); // ALICE + MIKE = (50 + 25) + 100  -> 175
        })
    }

    #[test]
    fn backers_of_success_for_direct() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(10);
            Pallet::enroll(&MIKE, STANDARD_VALUE, Fortitude::Force).unwrap();

            // BOB backed ALICE with 50 units
            Pallet::fund(
                &ALICE,
                &Funder::Direct(BOB),
                STANDARD_VALUE,
                Precision::BestEffort,
                Fortitude::Force,
            )
            .unwrap();

            // CHARLIE backed ALICE with 100 units
            Pallet::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                LARGE_VALUE,
                Precision::BestEffort,
                Fortitude::Force,
            )
            .unwrap();

            // MIKE backed ALICE with 25 units
            Pallet::fund(
                &ALICE,
                &Funder::Direct(MIKE),
                SMALL_VALUE,
                Precision::BestEffort,
                Fortitude::Force,
            )
            .unwrap();

            let actual_backers_of = Pallet::backers_of(&ALICE).unwrap();
            let expected_backers_of = vec![
                (Funder::Direct(BOB), 50),
                (Funder::Direct(MIKE), 25),
                (Funder::Direct(CHARLIE), 100),
            ];
            assert_eq!(actual_backers_of, expected_backers_of);
        })
    }

    #[test]
    fn backers_of_success_for_index() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            Pallet::enroll(&BOB, STANDARD_VALUE, Fortitude::Force).unwrap();

            Pallet::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                STANDARD_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            Pallet::fund(
                &BOB,
                &Funder::Direct(ALAN),
                LARGE_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let alice_digest = gen_author_digest(&ALICE).unwrap();
            let bob_digest = gen_author_digest(&BOB).unwrap();
            let entries = vec![(alice_digest.clone(), 60), (bob_digest.clone(), 40)];

            prepare_and_initiate_index(MIKE, FUNDING.into(), &entries, INDEX_DIGEST).unwrap();

            let by = Funder::Index {
                digest: INDEX_DIGEST,
                backer: MIKE,
            };
            Pallet::fund(&ALICE, &by, LARGE_VALUE, Precision::Exact, Fortitude::Force).unwrap();

            let backers_of_alice = Pallet::backers_of(&ALICE).unwrap();
            let expected_backers_of_alice = vec![(by.clone(), 60), (Funder::Direct(CHARLIE), 50)];
            assert_eq!(backers_of_alice, expected_backers_of_alice);

            let backers_of_bob = Pallet::backers_of(&BOB).unwrap();
            let expected_backers_of_bob = vec![(by, 40), (Funder::Direct(ALAN), 100)];
            assert_eq!(backers_of_bob, expected_backers_of_bob);
        })
    }

    #[test]
    fn backers_of_success_for_pool() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            Pallet::enroll(&BOB, STANDARD_VALUE, Fortitude::Force).unwrap();

            Pallet::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                STANDARD_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            Pallet::fund(
                &BOB,
                &Funder::Direct(ALAN),
                LARGE_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let alice_digest = gen_author_digest(&ALICE).unwrap();
            let bob_digest = gen_author_digest(&BOB).unwrap();
            let entries = vec![(alice_digest.clone(), 60), (bob_digest.clone(), 40)];

            prepare_and_initiate_pool(
                MIKE,
                FUNDING.into(),
                &entries,
                INDEX_DIGEST,
                POOL_DIGEST,
                Perbill::from_percent(5),
            )
            .unwrap();

            let by = Funder::Pool {
                digest: POOL_DIGEST,
                backer: MIKE,
            };

            Pallet::fund(&ALICE, &by, LARGE_VALUE, Precision::Exact, Fortitude::Force).unwrap();

            let backers_of_alice = Pallet::backers_of(&ALICE).unwrap();
            let expected_backers_of_alice = vec![(by.clone(), 60), (Funder::Direct(CHARLIE), 50)];
            assert_eq!(backers_of_alice, expected_backers_of_alice);

            let backers_of_bob = Pallet::backers_of(&BOB).unwrap();
            let expected_backers_of_bob = vec![(by, 40), (Funder::Direct(ALAN), 100)];
            assert_eq!(backers_of_bob, expected_backers_of_bob);
        })
    }

    #[test]
    fn backed_for_success_for_direct() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(10);
            Pallet::enroll(&MIKE, STANDARD_VALUE, Fortitude::Force).unwrap();

            // BOB backed ALICE with 50 units
            Pallet::fund(
                &ALICE,
                &Funder::Direct(BOB),
                STANDARD_VALUE,
                Precision::BestEffort,
                Fortitude::Force,
            )
            .unwrap();

            let current_total_backed_val = Pallet::total_backing();
            assert_eq!(current_total_backed_val, 50);

            // CHARLIE backed MIKE with 100 units
            Pallet::fund(
                &MIKE,
                &Funder::Direct(CHARLIE),
                LARGE_VALUE,
                Precision::BestEffort,
                Fortitude::Force,
            )
            .unwrap();

            let bob_backed_for = Pallet::backed_for(&Funder::Direct(BOB)).unwrap();
            let expected_author = vec![(ALICE, 50)];
            assert_eq!(bob_backed_for, expected_author);

            let charlie_backed_for = Pallet::backed_for(&Funder::Direct(CHARLIE)).unwrap();
            let expected_author = vec![(MIKE, 100)];
            assert_eq!(charlie_backed_for, expected_author);
        })
    }

    #[test]
    fn backed_for_success_for_index() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            Pallet::enroll(&BOB, STANDARD_VALUE, Fortitude::Force).unwrap();

            Pallet::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                STANDARD_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            Pallet::fund(
                &BOB,
                &Funder::Direct(ALAN),
                LARGE_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let alice_digest = gen_author_digest(&ALICE).unwrap();
            let bob_digest = gen_author_digest(&BOB).unwrap();
            let entries = vec![(alice_digest.clone(), 60), (bob_digest.clone(), 40)];

            prepare_and_initiate_index(MIKE, FUNDING.into(), &entries, INDEX_DIGEST).unwrap();

            let by = Funder::Index {
                digest: INDEX_DIGEST,
                backer: MIKE,
            };
            Pallet::fund(&ALICE, &by, LARGE_VALUE, Precision::Exact, Fortitude::Force).unwrap();

            let backed_for = Pallet::backed_for(&by).unwrap();
            let expected_backed_for = vec![(ALICE, 60), (BOB, 40)];
            assert_eq!(backed_for, expected_backed_for);
        })
    }

    #[test]
    fn backed_for_success_for_pool() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            Pallet::enroll(&BOB, STANDARD_VALUE, Fortitude::Force).unwrap();

            Pallet::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                STANDARD_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            Pallet::fund(
                &BOB,
                &Funder::Direct(ALAN),
                LARGE_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let alice_digest = gen_author_digest(&ALICE).unwrap();
            let bob_digest = gen_author_digest(&BOB).unwrap();
            let entries = vec![(alice_digest.clone(), 60), (bob_digest.clone(), 40)];

            prepare_and_initiate_pool(
                MIKE,
                FUNDING.into(),
                &entries,
                INDEX_DIGEST,
                POOL_DIGEST,
                Perbill::from_percent(5),
            )
            .unwrap();

            let by = Funder::Pool {
                digest: POOL_DIGEST,
                backer: MIKE,
            };
            Pallet::fund(&ALICE, &by, LARGE_VALUE, Precision::Exact, Fortitude::Force).unwrap();

            let backed_for = Pallet::backed_for(&by).unwrap();
            let expected_backed_for = vec![(ALICE, 60), (BOB, 40)];
            assert_eq!(backed_for, expected_backed_for);
        })
    }

    #[test]
    fn get_fund_success_for_direct() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(10);
            Pallet::enroll(&MIKE, STANDARD_VALUE, Fortitude::Force).unwrap();

            // BOB backed ALICE with 50 units
            Pallet::fund(
                &ALICE,
                &Funder::Direct(BOB),
                STANDARD_VALUE,
                Precision::BestEffort,
                Fortitude::Force,
            )
            .unwrap();

            let current_total_backed_val = Pallet::total_backing();
            assert_eq!(current_total_backed_val, 50);

            // CHARLIE backed MIKE with 100 units
            Pallet::fund(
                &MIKE,
                &Funder::Direct(CHARLIE),
                LARGE_VALUE,
                Precision::BestEffort,
                Fortitude::Force,
            )
            .unwrap();

            // BOB increase the backing by 25 units
            Pallet::fund(
                &ALICE,
                &Funder::Direct(BOB),
                SMALL_VALUE,
                Precision::BestEffort,
                Fortitude::Force,
            )
            .unwrap();

            let bobs_fund_to_alice = Pallet::get_fund(&ALICE, &Funder::Direct(BOB)).unwrap();
            assert_eq!(bobs_fund_to_alice, 75); // 50 + 25

            let charlies_fund_to_mike = Pallet::get_fund(&MIKE, &Funder::Direct(CHARLIE)).unwrap();
            assert_eq!(charlies_fund_to_mike, 100);
        })
    }

    #[test]
    fn get_fund_err_author_not_found() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(10);
            Pallet::enroll(&ALAN, STANDARD_VALUE, Fortitude::Force).unwrap();

            Pallet::fund(
                &ALICE,
                &Funder::Direct(BOB),
                LARGE_VALUE,
                Precision::BestEffort,
                Fortitude::Force,
            )
            .unwrap();

            assert_err!(
                Pallet::get_fund(&ALAN, &Funder::Direct(BOB)),
                Error::FundedToAnotherDigest
            );
        })
    }

    #[test]
    fn get_fund_success_for_index() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(10);
            Pallet::enroll(&MIKE, STANDARD_VALUE, Fortitude::Force).unwrap();

            // BOB backed ALICE with 50 units
            Pallet::fund(
                &ALICE,
                &Funder::Direct(BOB),
                STANDARD_VALUE,
                Precision::BestEffort,
                Fortitude::Force,
            )
            .unwrap();

            let current_total_backed_val = Pallet::total_backing();
            assert_eq!(current_total_backed_val, 50);

            // CHARLIE backed MIKE with 100 units
            Pallet::fund(
                &MIKE,
                &Funder::Direct(CHARLIE),
                LARGE_VALUE,
                Precision::BestEffort,
                Fortitude::Force,
            )
            .unwrap();

            let alice_digest = gen_author_digest(&ALICE).unwrap();
            let mike_digest = gen_author_digest(&MIKE).unwrap();
            let entries = vec![(alice_digest.clone(), 60), (mike_digest.clone(), 40)];

            // ALAN initates a index with both authors as an entry and funds the index
            prepare_and_initiate_index(ALAN, FUNDING.into(), &entries, INDEX_DIGEST).unwrap();

            let by = Funder::Index {
                digest: INDEX_DIGEST,
                backer: ALAN,
            };
            Pallet::fund(&ALICE, &by, LARGE_VALUE, Precision::Exact, Fortitude::Force).unwrap();

            let bobs_fund_to_index_alice = Pallet::get_fund(&ALICE, &by).unwrap();
            assert_eq!(bobs_fund_to_index_alice, 60);
            let bobs_fund_to_index_bob = Pallet::get_fund(&MIKE, &by).unwrap();
            assert_eq!(bobs_fund_to_index_bob, 40);
        })
    }

    #[test]
    fn get_fund_success_for_pool() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            Pallet::enroll(&BOB, STANDARD_VALUE, Fortitude::Force).unwrap();

            Pallet::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                STANDARD_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            Pallet::fund(
                &BOB,
                &Funder::Direct(ALAN),
                LARGE_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let alice_digest = gen_author_digest(&ALICE).unwrap();
            let bob_digest = gen_author_digest(&BOB).unwrap();
            let entries = vec![(alice_digest.clone(), 60), (bob_digest.clone(), 40)];

            prepare_and_initiate_pool(
                MIKE,
                FUNDING.into(),
                &entries,
                INDEX_DIGEST,
                POOL_DIGEST,
                Perbill::from_percent(5),
            )
            .unwrap();

            let by = Funder::Pool {
                digest: POOL_DIGEST,
                backer: MIKE,
            };

            Pallet::fund(&ALICE, &by, LARGE_VALUE, Precision::Exact, Fortitude::Force).unwrap();

            let bobs_fund_to_index_alice = Pallet::get_fund(&ALICE, &by).unwrap();
            assert_eq!(bobs_fund_to_index_alice, 60);
            let bobs_fund_to_index_bob = Pallet::get_fund(&BOB, &by).unwrap();
            assert_eq!(bobs_fund_to_index_bob, 40);
        })
    }

    #[test]
    fn fund_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            // BOB backed ALICE with 50 units
            assert_ok!(Pallet::fund(
                &ALICE,
                &Funder::Direct(BOB),
                STANDARD_VALUE,
                Precision::BestEffort,
                Fortitude::Force,
            ));

            let funds_backed = Pallet::get_fund(&ALICE, &Funder::Direct(BOB)).unwrap();
            assert_eq!(funds_backed, STANDARD_VALUE);

            // Raise backing by 25 units
            assert_ok!(Pallet::fund(
                &ALICE,
                &Funder::Direct(BOB),
                SMALL_VALUE,
                Precision::BestEffort,
                Fortitude::Force,
            ));

            let funds_backed = Pallet::get_fund(&ALICE, &Funder::Direct(BOB)).unwrap();
            assert_eq!(funds_backed, 75); // 50 + 25

            let author_funders = AuthorFunders::get((ALICE, BOB)).unwrap();
            assert_eq!(author_funders, Funder::Direct(BOB));
            System::assert_last_event(Event::AuthorFunded { author: ALICE, backer: BOB, amount: SMALL_VALUE }.into());
        })
    }

    #[test]
    fn fund_success_for_index() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(8);
            Pallet::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                STANDARD_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            System::set_block_number(12);
            Pallet::enroll(&BOB, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(15);
            Pallet::fund(
                &BOB,
                &Funder::Direct(ALAN),
                LARGE_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let total_backing = Pallet::total_backing();
            assert_eq!(total_backing, 150);
            let alice_backed_value = Pallet::backed_value(&ALICE).unwrap();
            assert_eq!(alice_backed_value, 50);
            let bob_backed_value = Pallet::backed_value(&BOB).unwrap();
            assert_eq!(bob_backed_value, 100);

            let alice_digest = gen_author_digest(&ALICE).unwrap();
            let bob_digest = gen_author_digest(&BOB).unwrap();
            let entries = vec![(alice_digest.clone(), 60), (bob_digest.clone(), 40)];

            let alice_current_hold = Pallet::get_hold(&ALICE).unwrap();
            assert_eq!(alice_current_hold, 100);
            let bob_current_hold = Pallet::get_hold(&BOB).unwrap();
            assert_eq!(bob_current_hold, 150);

            prepare_and_initiate_index(MIKE, FUNDING.into(), &entries, INDEX_DIGEST).unwrap();

            let by = Funder::Index {
                digest: INDEX_DIGEST,
                backer: MIKE,
            };

            assert_ok!(Pallet::fund(
                &ALICE,
                &by,
                LARGE_VALUE,
                Precision::Exact,
                Fortitude::Force,
            ));

            let total_backing = Pallet::total_backing();
            assert_eq!(total_backing, 250);
            let alice_backed_value = Pallet::backed_value(&ALICE).unwrap();
            assert_eq!(alice_backed_value, 110); // 50 (existing) + 60 (through index as ALICE share is 60 )
            let bob_backed_value = Pallet::backed_value(&BOB).unwrap();
            assert_eq!(bob_backed_value, 140); // 100 (existing) + 40 (through index as BOB share is 40 )

            let author_funders = AuthorFunders::get((ALICE, MIKE)).unwrap();
            assert_eq!(author_funders, by);

            let alice_current_hold = Pallet::get_hold(&ALICE).unwrap();
            assert_eq!(alice_current_hold, 160);
            let bob_current_hold = Pallet::get_hold(&BOB).unwrap();
            assert_eq!(bob_current_hold, 190);
            System::assert_last_event(Event::IndexFunded { index: INDEX_DIGEST, backer: MIKE, amount: LARGE_VALUE }.into());
        })
    }

    #[test]
    fn fund_success_for_pool() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(8);
            Pallet::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                STANDARD_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            System::set_block_number(12);
            Pallet::enroll(&BOB, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(15);
            Pallet::fund(
                &BOB,
                &Funder::Direct(ALAN),
                LARGE_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let total_backing = Pallet::total_backing();
            assert_eq!(total_backing, 150);
            let alice_backed_value = Pallet::backed_value(&ALICE).unwrap();
            assert_eq!(alice_backed_value, 50);
            let bob_backed_value = Pallet::backed_value(&BOB).unwrap();
            assert_eq!(bob_backed_value, 100);

            let alice_digest = gen_author_digest(&ALICE).unwrap();
            let bob_digest = gen_author_digest(&BOB).unwrap();
            let entries = vec![(alice_digest.clone(), 60), (bob_digest.clone(), 40)];

            let alice_current_hold = Pallet::get_hold(&ALICE).unwrap();
            assert_eq!(alice_current_hold, 100);
            let bob_current_hold = Pallet::get_hold(&BOB).unwrap();
            assert_eq!(bob_current_hold, 150);

            prepare_and_initiate_pool(
                ALAN,
                FUNDING.into(),
                &entries,
                INDEX_DIGEST,
                POOL_DIGEST,
                Perbill::from_percent(5),
            )
            .unwrap();

            let by = Funder::Pool {
                digest: POOL_DIGEST,
                backer: MIKE,
            };

            assert_ok!(Pallet::fund(
                &ALICE,
                &by,
                LARGE_VALUE,
                Precision::Exact,
                Fortitude::Force,
            ));

            let total_backing = Pallet::total_backing();
            assert_eq!(total_backing, 250);
            let alice_backed_value = Pallet::backed_value(&ALICE).unwrap();
            assert_eq!(alice_backed_value, 110); // 50 (existing) + 60 (through index as ALICE share is 60 )
            let bob_backed_value = Pallet::backed_value(&BOB).unwrap();
            assert_eq!(bob_backed_value, 140); // 100 (existing) + 40 (through index as BOB share is 40 )

            let author_funders = AuthorFunders::get((ALICE, MIKE)).unwrap();
            assert_eq!(author_funders, by);

            let alice_current_hold = Pallet::get_hold(&ALICE).unwrap();
            assert_eq!(alice_current_hold, 160);
            let bob_current_hold = Pallet::get_hold(&BOB).unwrap();
            assert_eq!(bob_current_hold, 190);
            System::assert_last_event(Event::PoolFunded { pool: POOL_DIGEST, backer: MIKE, amount: LARGE_VALUE }.into());
        })
    }

    #[test]
    fn draw_success_for_direct() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            // BOB backed ALICE with 50 units
            assert_ok!(Pallet::fund(
                &ALICE,
                &Funder::Direct(BOB),
                STANDARD_VALUE,
                Precision::BestEffort,
                Fortitude::Force,
            ));

            let current_backed_value = Pallet::backed_value(&ALICE).unwrap();
            assert_eq!(current_backed_value, STANDARD_VALUE);

            let alice_backers = Pallet::backers_of(&ALICE).unwrap();
            let expected_backers = vec![(Funder::Direct(BOB), STANDARD_VALUE)];
            assert_eq!(alice_backers, expected_backers);

            // withdraw the backed funds
            assert_ok!(Pallet::draw(&ALICE, &Funder::Direct(BOB)));

            let current_backed_value = Pallet::backed_value(&ALICE).unwrap();
            assert_eq!(current_backed_value, 0);

            let alice_backers = Pallet::backers_of(&ALICE).unwrap();
            let expected_backers = vec![];
            assert_eq!(alice_backers, expected_backers);
            assert!(!AuthorFunders::contains_key((ALICE, BOB)));
            System::assert_last_event(Event::AuthorDrawn { author: ALICE, backer: BOB, amount: STANDARD_VALUE }.into());
        })
    }

    #[test]
    fn draw_success_for_index() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(8);
            Pallet::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                STANDARD_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            System::set_block_number(12);
            Pallet::enroll(&BOB, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(15);
            Pallet::fund(
                &BOB,
                &Funder::Direct(ALAN),
                LARGE_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let alice_digest = gen_author_digest(&ALICE).unwrap();
            let bob_digest = gen_author_digest(&BOB).unwrap();
            let entries = vec![(alice_digest.clone(), 60), (bob_digest.clone(), 40)];

            prepare_and_initiate_index(MIKE, FUNDING.into(), &entries, INDEX_DIGEST).unwrap();

            let by = Funder::Index {
                digest: INDEX_DIGEST,
                backer: MIKE,
            };

            Pallet::fund(&ALICE, &by, LARGE_VALUE, Precision::Exact, Fortitude::Force).unwrap();

            let total_backing = Pallet::total_backing();
            assert_eq!(total_backing, 250);
            let alice_backed_value = Pallet::backed_value(&ALICE).unwrap();
            assert_eq!(alice_backed_value, 110); // 50 (existing) + 60 (through index as ALICE share is 60 )
            let bob_backed_value = Pallet::backed_value(&BOB).unwrap();
            assert_eq!(bob_backed_value, 140); // 100 (existing) + 40 (through index as BOB share is 40 )

            let author_funders = AuthorFunders::get((ALICE, MIKE)).unwrap();
            assert_eq!(author_funders, by);

            assert_ok!(Pallet::draw(&ALICE, &by));

            let total_backing = Pallet::total_backing();
            assert_eq!(total_backing, 150);
            let alice_backed_value = Pallet::backed_value(&ALICE).unwrap();
            assert_eq!(alice_backed_value, 50); // 50 (existing) - 60 (through index as ALICE share is 60 )
            let bob_backed_value = Pallet::backed_value(&BOB).unwrap();
            assert_eq!(bob_backed_value, 100); // 100 (existing) - 40 (through index as BOB share is 40 )

            assert!(!AuthorFunders::contains_key((ALICE, MIKE)));

            let mike_balance = get_user_balance(&MIKE);
            assert_eq!(mike_balance, 200);
            System::assert_last_event(Event::IndexDrawn { index: INDEX_DIGEST, backer: MIKE, amount: LARGE_VALUE }.into());
        })
    }

    #[test]
    fn draw_success_for_pool() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(8);
            Pallet::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                STANDARD_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            System::set_block_number(12);
            Pallet::enroll(&BOB, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(15);
            Pallet::fund(
                &BOB,
                &Funder::Direct(ALAN),
                LARGE_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let alice_digest = gen_author_digest(&ALICE).unwrap();
            let bob_digest = gen_author_digest(&BOB).unwrap();
            let entries = vec![(alice_digest.clone(), 60), (bob_digest.clone(), 40)];

            prepare_and_initiate_pool(
                ALAN,
                FUNDING.into(),
                &entries,
                INDEX_DIGEST,
                POOL_DIGEST,
                Perbill::from_percent(5),
            )
            .unwrap();

            let by = Funder::Pool {
                digest: POOL_DIGEST,
                backer: MIKE,
            };

            Pallet::fund(&ALICE, &by, LARGE_VALUE, Precision::Exact, Fortitude::Force).unwrap();

            let total_backing = Pallet::total_backing();
            assert_eq!(total_backing, 250);
            let alice_backed_value = Pallet::backed_value(&ALICE).unwrap();
            assert_eq!(alice_backed_value, 110); // 50 (existing) + 60 (through pool as ALICE share is 60 )
            let bob_backed_value = Pallet::backed_value(&BOB).unwrap();
            assert_eq!(bob_backed_value, 140); // 100 (existing) + 40 (through pool as BOB share is 40 )

            let author_funders = AuthorFunders::get((ALICE, MIKE)).unwrap();
            assert_eq!(author_funders, by);
            let author_funders = AuthorFunders::get((BOB, MIKE)).unwrap();
            assert_eq!(author_funders, by);

            assert_ok!(Pallet::draw(&ALICE, &by));

            let total_backing = Pallet::total_backing();
            assert_eq!(total_backing, 150);
            let alice_backed_value = Pallet::backed_value(&ALICE).unwrap();
            assert_eq!(alice_backed_value, 50); // 50 (existing) - 60 (through index as ALICE share is 60 )
            let bob_backed_value = Pallet::backed_value(&BOB).unwrap();
            assert_eq!(bob_backed_value, 100); // 100 (existing) - 40 (through index as BOB share is 40 )

            assert!(!AuthorFunders::contains_key((ALICE, MIKE)));

            let mike_balance = get_user_balance(&MIKE);
            assert_eq!(mike_balance, 195); // 100 (existing) + 100 (backed) - 5 (commission)
            let alan_balance = get_user_balance(&ALAN);
            assert_eq!(alan_balance, 105); // 100 (existing) + 5 (commission)
            System::assert_last_event(Event::PoolDrawn { pool: POOL_DIGEST, backer: MIKE, amount: 95 }.into());
        })
    }

    #[test]
    fn on_drawn_direct_success() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            let draw_amount = STANDARD_VALUE;
            let by = Funder::<Test>::Direct(ALICE);
            Pallet::on_drawn(&ALAN, &by, draw_amount);

            System::assert_last_event(
                Event::AuthorDrawn {
                    author: ALAN,
                    backer: ALICE,
                    amount: draw_amount,
                }
                .into(),
            );
        })
    }

    #[test]
    fn on_drawn_index_success() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            let draw_amount = STANDARD_VALUE;
            let by = Funder::<Test>::Index {
                digest: INDEX_DIGEST,
                backer: ALICE,
            };
            Pallet::on_drawn(&ALAN, &by, draw_amount);

            System::assert_last_event(
                Event::IndexDrawn {
                    index: INDEX_DIGEST,
                    backer: ALICE,
                    amount: draw_amount,
                }
                .into(),
            );
        })
    }

    #[test]
    fn on_drawn_pool_success() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            let draw_amount = STANDARD_VALUE;
            let by = Funder::<Test>::Pool {
                digest: POOL_DIGEST,
                backer: ALICE,
            };
            Pallet::on_drawn(&ALAN, &by, draw_amount);

            System::assert_last_event(
                Event::PoolDrawn {
                    pool: POOL_DIGEST,
                    backer: ALICE,
                    amount: draw_amount,
                }
                .into(),
            );
        })
    }

    #[test]
    fn on_funded_direct_success() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            let fund_amount = STANDARD_VALUE;
            let by = Funder::<Test>::Direct(ALICE);
            Pallet::on_funded(&ALAN, &by, fund_amount);

            System::assert_last_event(
                Event::AuthorFunded {
                    author: ALAN,
                    backer: ALICE,
                    amount: fund_amount,
                }
                .into(),
            );
        })
    }

    #[test]
    fn on_funded_index_success() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            let draw_amount = STANDARD_VALUE;
            let by = Funder::<Test>::Index {
                digest: INDEX_DIGEST,
                backer: ALICE,
            };
            Pallet::on_funded(&ALAN, &by, draw_amount);

            System::assert_last_event(
                Event::IndexFunded {
                    index: INDEX_DIGEST,
                    backer: ALICE,
                    amount: draw_amount,
                }
                .into(),
            );
        })
    }

    #[test]
    fn on_funded_pool_success() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            let draw_amount = STANDARD_VALUE;
            let by = Funder::<Test>::Pool {
                digest: POOL_DIGEST,
                backer: ALICE,
            };
            Pallet::on_funded(&ALAN, &by, draw_amount);

            System::assert_last_event(
                Event::PoolFunded {
                    pool: POOL_DIGEST,
                    backer: ALICE,
                    amount: draw_amount,
                }
                .into(),
            );
        })
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // `````````````````````````````` COMPENSATE ROLES ```````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn reward_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(10);
            let reward_units = 5;
            let reward_block = Pallet::reward(&ALICE, reward_units, Precision::BestEffort).unwrap();
            // First reward is scheduled at block 12 (current block 10 + buffer 2)
            assert_eq!(reward_block, 12);
            let reward_scheduled = AuthorRewards::get((12, ALICE)).unwrap();
            assert_eq!(reward_scheduled, 5);

            // Scheduling a second reward at the same block results in automatic
            // collision avoidance: the slot at block 12 is occupied, so the reward
            // is deferred to the next available block
            let reward_units = 10;
            let reward_block = Pallet::reward(&ALICE, reward_units, Precision::BestEffort).unwrap();

            // Second reward is scheduled at block 13 due to slot collision
            assert_eq!(reward_block, 13);
            let reward_scheduled = AuthorRewards::get((13, ALICE)).unwrap();
            assert_eq!(reward_scheduled, 10);
            System::assert_last_event(Event::AuthorRewardScheduled { author: ALICE, amount: reward_units, at: reward_block }.into());
        })
    }

    #[test]
    fn reward_err_author_resigned() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            AuthorsMap::mutate(ALICE, |author| {
                let info = author.as_mut().unwrap();
                let status = &mut info.status;
                *status = AuthorStatus::Resigned;
            });

            let reward_unit = 5;
            assert_err!(
                Pallet::reward(&ALICE, reward_unit, Precision::BestEffort,),
                Error::AuthorResigned
            );
        })
    }

    #[test]
    fn reclaim_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(10);
            let reward_units = 5;
            let reward_block = Pallet::reward(&ALICE, reward_units, Precision::BestEffort).unwrap();
            // First reward is scheduled at block 12 (current block 10 + buffer 2)
            assert_eq!(reward_block, 12);
            let reward_scheduled = AuthorRewards::get((12, ALICE)).unwrap();
            assert_eq!(reward_scheduled, 5);

            System::set_block_number(11);
            assert_ok!(Pallet::reclaim(&ALICE, 12));
            assert!(!AuthorRewards::contains_key((12, ALICE)));
            System::assert_last_event(Event::AuthorRewardReclaimed { author: ALICE, amount: reward_units}.into());
        })
    }

    #[test]
    fn reclaim_err_finalized_obligations() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(10);
            let reward_units = 5;
            let reward_block = Pallet::reward(&ALICE, reward_units, Precision::BestEffort).unwrap();
            assert_eq!(reward_block, 12);

            System::set_block_number(12);
            assert_err!(Pallet::reclaim(&ALICE, 12), Error::FinalizedObligations);
        })
    }

    #[test]
    fn reclaim_err_rewards_not_founds() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(10);
            assert_err!(Pallet::reclaim(&ALICE, 12), Error::RewardNotFound);
        })
    }

    #[test]
    fn penalize_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            let meta = Pallet::get_meta(&ALICE).unwrap();
            assert_eq!(meta.risk_until, 6);

            System::set_block_number(10);
            let penalty_block = Pallet::penalize(&ALICE, Perbill::from_percent(5)).unwrap();
            // First penalty is scheduled at block 14 (current block 10 + buffer 4)
            assert_eq!(penalty_block, 14);
            let penalty_scheduled = AuthorPenalties::get((14, ALICE)).unwrap();
            assert_eq!(penalty_scheduled, Perbill::from_percent(5));
            // Author's risk period was extended by the IncreaseProbationBy value (1 block)
            let meta = Pallet::get_meta(&ALICE).unwrap();
            assert_eq!(meta.risk_until, 11);

            // Scheduling a second penalty at the same block results in automatic
            // collision avoidance: the slot at block 14 is occupied, so the penalty
            // is deferred to the next available block
            let penalty_block = Pallet::penalize(&ALICE, Perbill::from_percent(10)).unwrap();
            // Second penalty is scheduled at block 15 due to slot collision
            assert_eq!(penalty_block, 15);
            let penalty_scheduled = AuthorPenalties::get((15, ALICE)).unwrap();
            assert_eq!(penalty_scheduled, Perbill::from_percent(10));
            // Risk period is extended again (now 11 + 1 = 12)
            let meta = Pallet::get_meta(&ALICE).unwrap();
            assert_eq!(meta.risk_until, 12);
            System::assert_last_event(Event::AuthorPenaltyScheduled { author: ALICE, factor: penalty_scheduled, at: 15 }.into());
        })
    }

    #[test]
    fn penalize_err_zero_penalty_found() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(10);
            assert_err!(
                Pallet::penalize(&ALICE, Perbill::from_percent(0)),
                Error::ZeroPenaltyFound
            );
        })
    }

    #[test]
    fn penalize_err_author_resigned() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            AuthorsMap::mutate(ALICE, |author| {
                let info = author.as_mut().unwrap();
                let status = &mut info.status;
                *status = AuthorStatus::Resigned;
            });

            System::set_block_number(10);
            assert_err!(
                Pallet::penalize(&ALICE, Perbill::from_percent(5)),
                Error::AuthorResigned
            );
        })
    }

    #[test]
    fn forgive_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(10);
            let penalty_factor = Perbill::from_percent(5);
            let penalty_block = Pallet::penalize(&ALICE, penalty_factor).unwrap();

            assert_eq!(penalty_block, 14);
            let penalty_scheduled = AuthorPenalties::get((14, ALICE)).unwrap();
            assert_eq!(penalty_scheduled, penalty_factor);

            let meta = Pallet::get_meta(&ALICE).unwrap();
            assert_eq!(meta.risk_until, 11);

            assert_ok!(Pallet::forgive(&ALICE, 14));

            assert!(!AuthorPenalties::contains_key((14, ALICE)));

            let meta = Pallet::get_meta(&ALICE).unwrap();
            assert_eq!(meta.risk_until, 10);
            System::assert_last_event(Event::AuthorPenaltyForgiven { author: ALICE, factor: penalty_scheduled }.into());
        })
    }

    #[test]
    fn forgive_err_finalized_obligations() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(10);
            let penalty_factor = Perbill::from_percent(5);
            let penalty_block = Pallet::penalize(&ALICE, penalty_factor).unwrap();

            assert_eq!(penalty_block, 14);
            let penalty_scheduled = AuthorPenalties::get((14, ALICE)).unwrap();
            assert_eq!(penalty_scheduled, penalty_factor);

            System::set_block_number(14);
            assert_err!(Pallet::forgive(&ALICE, 14), Error::FinalizedObligations);
        })
    }

    #[test]
    fn forgive_err_rewards_not_founds() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(10);
            assert_err!(Pallet::forgive(&ALICE, 14), Error::PenaltyNotFound);
        })
    }

    #[test]
    fn has_reward_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(10);
            let reward_units = 5;
            Pallet::reward(&ALICE, reward_units, Precision::BestEffort).unwrap();

            assert_ok!(Pallet::has_reward(&ALICE));
        })
    }

    #[test]
    fn has_reward_err_reward_not_found() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            assert_err!(Pallet::has_reward(&ALICE), Error::RewardNotFound);
        })
    }

    #[test]
    fn has_penalty_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(10);
            Pallet::penalize(&ALICE, Perbill::from_percent(5)).unwrap();

            assert_ok!(Pallet::has_penalty(&ALICE));
        })
    }

    #[test]
    fn has_penalty_penalty_not_found() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            assert_err!(Pallet::has_penalty(&ALICE), Error::PenaltyNotFound);
        })
    }

    #[test]
    fn get_rewards_of_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(10);
            let reward_units = 5;
            Pallet::reward(&ALICE, reward_units, Precision::BestEffort).unwrap();

            System::set_block_number(11);
            let reward_units = 10;
            Pallet::reward(&ALICE, reward_units, Precision::BestEffort).unwrap();

            let reward_units = 8;
            Pallet::reward(&ALICE, reward_units, Precision::BestEffort).unwrap();

            let rewards_of = Pallet::get_rewards_of(&ALICE).unwrap();
            let expected_rewards_of = vec![(12, 5), (13, 10), (14, 8)];
            assert_eq!(rewards_of, expected_rewards_of);
        })
    }

    #[test]
    fn get_rewards_of_success_with_empty_vec_when_no_rewards() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            let rewards_of = Pallet::get_rewards_of(&ALICE).unwrap();
            assert_eq!(rewards_of, vec![]);
        })
    }

    #[test]
    fn get_penalties_of_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(10);
            let penalty_factor = Perbill::from_percent(5);
            Pallet::penalize(&ALICE, penalty_factor).unwrap();

            System::set_block_number(11);
            let penalty_factor = Perbill::from_percent(10);
            Pallet::penalize(&ALICE, penalty_factor).unwrap();

            System::set_block_number(12);
            let penalty_factor = Perbill::from_percent(8);
            Pallet::penalize(&ALICE, penalty_factor).unwrap();

            let penalties_of = Pallet::get_penalties_of(&ALICE).unwrap();
            let expected_penalties_of = vec![
                (14, Perbill::from_percent(5)),
                (15, Perbill::from_percent(10)),
                (16, Perbill::from_percent(8)),
            ];
            assert_eq!(penalties_of, expected_penalties_of);
        })
    }

    #[test]
    fn get_penalties_of_success_with_empty_vec_when_no_penalty() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            let penalties_of = Pallet::get_penalties_of(&ALICE).unwrap();
            assert_eq!(penalties_of, vec![]);
        })
    }

    #[test]
    fn get_rewards_on_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(10);
            let reward_units = 5;
            Pallet::reward(&ALICE, reward_units, Precision::BestEffort).unwrap();

            System::set_block_number(11);
            let reward_units = 10;
            Pallet::reward(&ALICE, reward_units, Precision::BestEffort).unwrap();

            let reward_on_12 = Pallet::get_rewards_on(12).unwrap();
            let expected_reward_on_12 = vec![(ALICE, 5)];
            assert_eq!(reward_on_12, expected_reward_on_12);

            let reward_on_13 = Pallet::get_rewards_on(13).unwrap();
            let expected_reward_on_13 = vec![(ALICE, 10)];
            assert_eq!(reward_on_13, expected_reward_on_13);
        })
    }

    #[test]
    fn get_rewards_on_err_finalize_obligations() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(10);
            let reward_units = 5;
            Pallet::reward(&ALICE, reward_units, Precision::BestEffort).unwrap();

            System::set_block_number(11);
            let reward_units = 10;
            Pallet::reward(&ALICE, reward_units, Precision::BestEffort).unwrap();

            System::set_block_number(15);
            assert_err!(Pallet::get_rewards_on(12), Error::FinalizedObligations);
        })
    }

    #[test]
    fn get_rewards_on_err_contains_no_rewards() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(10);
            let reward_units = 5;
            Pallet::reward(&ALICE, reward_units, Precision::BestEffort).unwrap();

            let rewards_on = Pallet::get_rewards_on(13).unwrap();
            assert_eq!(rewards_on, vec![]);
        })
    }

    #[test]
    fn get_penalties_on_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(10);
            let penalty_factor = Perbill::from_percent(5);
            Pallet::penalize(&ALICE, penalty_factor).unwrap();

            System::set_block_number(11);
            let penalty_factor = Perbill::from_percent(10);
            Pallet::penalize(&ALICE, penalty_factor).unwrap();

            let penalty_on_12 = Pallet::get_penalties_on(14).unwrap();
            let expected_penalty_on_12 = vec![(ALICE, Perbill::from_percent(5))];
            assert_eq!(penalty_on_12, expected_penalty_on_12);

            let penalty_on_13 = Pallet::get_penalties_on(15).unwrap();
            let expected_penalty_on_13 = vec![(ALICE, Perbill::from_percent(10))];
            assert_eq!(penalty_on_13, expected_penalty_on_13);
        })
    }

    #[test]
    fn get_penalties_on_err_finalized_obligations() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(10);
            let penalty_factor = Perbill::from_percent(5);
            Pallet::penalize(&ALICE, penalty_factor).unwrap();

            System::set_block_number(11);
            let penalty_factor = Perbill::from_percent(10);
            Pallet::penalize(&ALICE, penalty_factor).unwrap();

            System::set_block_number(15);
            assert_err!(Pallet::get_penalties_on(14), Error::FinalizedObligations);
        })
    }

    #[test]
    fn get_penalties_on_success_with_empty_vec_when_no_penalty() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(10);
            let penalty_factor = Perbill::from_percent(5);
            Pallet::penalize(&ALICE, penalty_factor).unwrap();

            let penalties_on = Pallet::get_penalties_on(15).unwrap();
            assert_eq!(penalties_on, vec![]);
        })
    }

    #[test]
    fn get_hold_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            Pallet::fund(
                &ALICE,
                &Funder::Direct(BOB),
                LARGE_VALUE,
                Precision::BestEffort,
                Fortitude::Force,
            )
            .unwrap();

            let actual_hold = Pallet::get_hold(&ALICE).unwrap();
            let expected_hold = 150; // 50(collateral) + 100 (funding)
            assert_eq!(actual_hold, expected_hold);

            Pallet::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                SMALL_VALUE,
                Precision::BestEffort,
                Fortitude::Force,
            )
            .unwrap();

            let actual_hold = Pallet::get_hold(&ALICE).unwrap();
            let expected_hold = 175; // 50(collateral) + 100 (funding) + 25 (funding)
            assert_eq!(actual_hold, expected_hold);
        })
    }

    #[test]
    fn set_hold_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            Pallet::fund(
                &ALICE,
                &Funder::Direct(BOB),
                LARGE_VALUE,
                Precision::BestEffort,
                Fortitude::Force,
            )
            .unwrap();

            Pallet::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                SMALL_VALUE,
                Precision::BestEffort,
                Fortitude::Force,
            )
            .unwrap();

            let actual_hold = Pallet::get_hold(&ALICE).unwrap();
            let expected_hold = 175; // 50(collateral) + 100 (funding) + 25 (funding)
            assert_eq!(actual_hold, expected_hold);

            let collateral_value = Pallet::get_collateral(&ALICE).unwrap();
            let funding_value = Pallet::total_backing();
            assert_eq!(collateral_value, 50);
            assert_eq!(funding_value, 125);

            assert_ok!(Pallet::set_hold(
                &ALICE,
                250,
                Precision::Exact,
                Fortitude::Force
            ));

            let actual_hold = Pallet::get_hold(&ALICE).unwrap();
            let expected_hold = 250;
            // hold value updated to set_hold value
            assert_eq!(actual_hold, expected_hold);

            // collateral and funding value are updated accordingly
            let collateral_value = Pallet::get_collateral(&ALICE).unwrap();
            let funding_value = Pallet::total_backing();
            assert_eq!(collateral_value, 71);
            assert_eq!(funding_value, 179);
            System::assert_last_event(Event::AuthorTotalHold { author: ALICE, value: 250 }.into());
        })
    }

    #[test]
    fn on_reward_event_emmission_success() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            let at = System::block_number();
            let amount = LARGE_VALUE;
            Pallet::on_reward(&ALICE, amount, at);

            System::assert_last_event(
                AuthorRewardScheduled {
                    author: ALICE,
                    amount,
                    at,
                }
                .into(),
            )
        })
    }

    #[test]
    fn on_reclaim_event_emmission_success() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            let amount = LARGE_VALUE;
            Pallet::on_reclaim(&ALICE, amount);

            System::assert_last_event(
                AuthorRewardReclaimed {
                    author: ALICE,
                    amount,
                }
                .into(),
            )
        })
    }

    #[test]
    fn on_set_hold_event_emmission_success() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            let value = LARGE_VALUE;
            Pallet::on_set_hold(&ALICE, value);

            System::assert_last_event(
                AuthorTotalHold {
                    author: ALICE,
                    value,
                }
                .into(),
            )
        })
    }

    #[test]
    fn on_forgive_event_emmission_success() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            let factor = Perbill::from_percent(10);
            Pallet::on_forgive(&ALICE, factor);

            System::assert_last_event(
                AuthorPenaltyForgiven {
                    author: ALICE,
                    factor,
                }
                .into(),
            )
        })
    }

    #[test]
    fn on_penalize_event_emmission_success() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            let at = System::block_number();
            let factor = Perbill::from_percent(10);
            Pallet::on_penalize(&ALICE, factor, at);

            System::assert_last_event(
                AuthorPenaltyScheduled {
                    author: ALICE,
                    factor,
                    at,
                }
                .into(),
            )
        })
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````` ROLE PROBATION ````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn is_on_probation_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            assert_ok!(Pallet::is_on_probation(&ALICE));
        })
    }

    #[test]
    fn is_on_probation_err_author_is_active() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            AuthorsMap::mutate(ALICE, |author| {
                let info = author.as_mut().unwrap();
                let status = &mut info.status;
                *status = AuthorStatus::Active;
            });

            assert_err!(Pallet::is_on_probation(&ALICE), Error::AuthorIsActive);
        })
    }

    #[test]
    fn is_on_probation_err_author_is_resigned() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            AuthorsMap::mutate(ALICE, |author| {
                let info = author.as_mut().unwrap();
                let status = &mut info.status;
                *status = AuthorStatus::Resigned;
            });

            assert_err!(Pallet::is_on_probation(&ALICE), Error::AuthorResigned);
        })
    }

    #[test]
    fn is_permanent_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            AuthorsMap::mutate(ALICE, |author| {
                let info = author.as_mut().unwrap();
                let status = &mut info.status;
                *status = AuthorStatus::Active;
            });

            assert_ok!(Pallet::is_permanent(&ALICE));
        })
    }

    #[test]
    fn is_permanent_err_author_in_probation() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            assert_err!(Pallet::is_permanent(&ALICE), Error::AuthorInProbation);
        })
    }

    #[test]
    fn is_permanent_err_author_resigned() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            AuthorsMap::mutate(ALICE, |author| {
                let info = author.as_mut().unwrap();
                let status = &mut info.status;
                *status = AuthorStatus::Resigned;
            });

            assert_err!(Pallet::is_permanent(&ALICE), Error::AuthorResigned);
        })
    }

    #[test]
    fn can_be_permament_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(14);
            assert_err!(Pallet::can_be_permanent(&ALICE), Error::AuthorInProbation);

            System::set_block_number(16);
            assert_ok!(Pallet::can_be_permanent(&ALICE));
        })
    }

    #[test]
    fn can_be_permament_err_author_is_active() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            AuthorsMap::mutate(ALICE, |author| {
                let info = author.as_mut().unwrap();
                let status = &mut info.status;
                *status = AuthorStatus::Active;
            });

            System::set_block_number(16);
            assert_err!(Pallet::can_be_permanent(&ALICE), Error::AuthorIsActive);
        })
    }

    #[test]
    fn can_be_permament_err_author_resigned() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            AuthorsMap::mutate(ALICE, |author| {
                let info = author.as_mut().unwrap();
                let status = &mut info.status;
                *status = AuthorStatus::Resigned;
            });

            System::set_block_number(16);
            assert_err!(Pallet::can_be_permanent(&ALICE), Error::AuthorResigned);
        })
    }

    #[test]
    fn can_be_permament_err_author_not_found() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(16);
            assert_err!(Pallet::can_be_permanent(&BOB), Error::AuthorNotFound);
        })
    }

    #[test]
    fn can_be_permament_err_author_is_unsafe() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            AuthorsMap::mutate(ALICE, |author| {
                let info = author.as_mut().unwrap();
                let risk = &mut info.risk_until;
                *risk = 18;
            });

            System::set_block_number(16);
            assert_err!(Pallet::can_be_permanent(&ALICE), Error::AuthorIsUnsafe);
        })
    }

    #[test]
    fn risk_probation_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            let meta = Pallet::get_meta(&ALICE).unwrap();
            assert_eq!(meta.risk_until, 6);

            System::set_block_number(10);
            Pallet::risk_probation(&ALICE).unwrap();

            let meta = Pallet::get_meta(&ALICE).unwrap();
            assert_eq!(meta.risk_until, 11);

            Pallet::risk_probation(&ALICE).unwrap();

            let meta = Pallet::get_meta(&ALICE).unwrap();
            assert_eq!(meta.risk_until, 12);

            System::set_block_number(15);
            Pallet::risk_probation(&ALICE).unwrap();

            let meta = Pallet::get_meta(&ALICE).unwrap();
            assert_eq!(meta.risk_until, 16);
            System::assert_last_event(Event::AuthorAtRisk { author: ALICE, status: AuthorStatus::Probation, until: meta.risk_until }.into());
        })
    }

    #[test]
    fn risk_probation_err_author_not_found() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(10);
            assert_err!(Pallet::risk_probation(&BOB), Error::AuthorNotFound);
        })
    }

    #[test]
    fn risk_probation_err_author_is_active() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            AuthorsMap::mutate(ALICE, |author| {
                let info = author.as_mut().unwrap();
                let status = &mut info.status;
                *status = AuthorStatus::Active;
            });

            System::set_block_number(10);
            assert_err!(Pallet::risk_probation(&ALICE), Error::AuthorIsActive);
        })
    }

    #[test]
    fn risk_probation_err_author_resigned() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            AuthorsMap::mutate(ALICE, |author| {
                let info = author.as_mut().unwrap();
                let status = &mut info.status;
                *status = AuthorStatus::Resigned;
            });

            System::set_block_number(10);
            assert_err!(Pallet::risk_probation(&ALICE), Error::AuthorResigned);
        })
    }

    #[test]
    fn risk_permanence_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            let meta = Pallet::get_meta(&ALICE).unwrap();
            assert_eq!(meta.risk_until, 6);

            AuthorsMap::mutate(ALICE, |author| {
                let info = author.as_mut().unwrap();
                let status = &mut info.status;
                *status = AuthorStatus::Active;
            });

            System::set_block_number(20);
            Pallet::risk_permanence(&ALICE).unwrap();

            let meta = Pallet::get_meta(&ALICE).unwrap();
            assert_eq!(meta.risk_until, 21);

            Pallet::risk_permanence(&ALICE).unwrap();

            let meta = Pallet::get_meta(&ALICE).unwrap();
            assert_eq!(meta.risk_until, 22);

            System::set_block_number(25);
            Pallet::risk_permanence(&ALICE).unwrap();

            let meta = Pallet::get_meta(&ALICE).unwrap();
            assert_eq!(meta.risk_until, 26);
            System::assert_last_event(Event::AuthorAtRisk { author: ALICE, status: AuthorStatus::Active, until: meta.risk_until }.into());
        })
    }

    #[test]
    fn risk_permanence_success_revoking_permanence() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            let meta = Pallet::get_meta(&ALICE).unwrap();
            assert_eq!(meta.risk_until, 6);

            AuthorsMap::mutate(ALICE, |author| {
                let info = author.as_mut().unwrap();
                let status = &mut info.status;
                let risk_until = &mut info.risk_until;
                *status = AuthorStatus::Active;
                *risk_until = 31;
            });

            let meta = Pallet::get_meta(&ALICE).unwrap();
            assert_eq!(meta.status, AuthorStatus::Active);
            assert_eq!(meta.risk_until, 31);

            System::set_block_number(20);
            Pallet::risk_permanence(&ALICE).unwrap();

            let meta = Pallet::get_meta(&ALICE).unwrap();
            assert_eq!(meta.status, AuthorStatus::Active);
            assert_eq!(meta.risk_until, 32);
        })
    }

    #[test]
    fn risk_permanence_err_author_in_probation() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(10);
            assert_err!(Pallet::risk_permanence(&ALICE), Error::AuthorInProbation);
        })
    }

    #[test]
    fn risk_permanence_err_author_resigned() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            AuthorsMap::mutate(ALICE, |author| {
                let info = author.as_mut().unwrap();
                let status = &mut info.status;
                *status = AuthorStatus::Resigned;
            });

            System::set_block_number(35);
            assert_err!(Pallet::risk_permanence(&ALICE), Error::AuthorResigned);
        })
    }

    #[test]
    fn risk_permanence_err_author_not_found() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(10);
            assert_err!(Pallet::risk_permanence(&BOB), Error::AuthorNotFound);
        })
    }

    #[test]
    fn secure_permanence_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            let meta = Pallet::get_meta(&ALICE).unwrap();
            assert_eq!(meta.risk_until, 6);

            System::set_block_number(10);
            Pallet::risk_probation(&ALICE).unwrap();

            let meta = Pallet::get_meta(&ALICE).unwrap();
            assert_eq!(meta.risk_until, 11);

            assert_ok!(Pallet::secure_permanence(&ALICE));
            // risk is reduced when author under probation
            let meta = Pallet::get_meta(&ALICE).unwrap();
            assert_eq!(meta.risk_until, 10);

            System::set_block_number(20);
            AuthorsMap::mutate(ALICE, |author| {
                let info = author.as_mut().unwrap();
                let status = &mut info.status;
                *status = AuthorStatus::Active;
            });

            Pallet::risk_permanence(&ALICE).unwrap();

            let meta = Pallet::get_meta(&ALICE).unwrap();
            assert_eq!(meta.risk_until, 21);

            assert_ok!(Pallet::secure_permanence(&ALICE));
            // risk is reduced when author is Active
            let meta = Pallet::get_meta(&ALICE).unwrap();
            assert_eq!(meta.risk_until, 20);
            System::assert_last_event(Event::AuthorAtRisk { author: ALICE, status: AuthorStatus::Probation, until: meta.risk_until }.into());
        })
    }

    #[test]
    fn secure_permanence_err_author_resigned() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            AuthorsMap::mutate(ALICE, |author| {
                let info = author.as_mut().unwrap();
                let status = &mut info.status;
                *status = AuthorStatus::Resigned;
            });

            System::set_block_number(35);
            assert_err!(Pallet::secure_permanence(&ALICE), Error::AuthorResigned);
        })
    }

    #[test]
    fn set_permanence_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            let meta = Pallet::get_meta(&ALICE).unwrap();
            assert_eq!(meta.status, AuthorStatus::Probation);

            System::set_block_number(18);
            assert_ok!(Pallet::set_permanence(&ALICE));

            let meta = Pallet::get_meta(&ALICE).unwrap();
            assert_eq!(meta.status, AuthorStatus::Active);
            System::assert_last_event(Event::AuthorStatus { author: ALICE, status: AuthorStatus::Active }.into());
        })
    }

    #[test]
    fn set_permanence_author_not_found() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(18);
            assert_err!(Pallet::set_permanence(&BOB), Error::AuthorNotFound);
        })
    }

    #[test]
    fn revoke_permanance_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(16);
            Pallet::set_permanence(&ALICE).unwrap();

            let meta = Pallet::get_meta(&ALICE).unwrap();
            assert_eq!(meta.status, AuthorStatus::Active);

            System::set_block_number(20);
            Pallet::risk_permanence(&ALICE).unwrap();
            Pallet::risk_permanence(&ALICE).unwrap();
            Pallet::risk_permanence(&ALICE).unwrap();
            Pallet::risk_permanence(&ALICE).unwrap();
            Pallet::risk_permanence(&ALICE).unwrap();
            Pallet::risk_permanence(&ALICE).unwrap();
            Pallet::risk_permanence(&ALICE).unwrap();
            Pallet::risk_permanence(&ALICE).unwrap();
            Pallet::risk_permanence(&ALICE).unwrap();
            Pallet::risk_permanence(&ALICE).unwrap();
            Pallet::risk_permanence(&ALICE).unwrap();

            let meta = Pallet::get_meta(&ALICE).unwrap();
            assert_eq!(meta.risk_until, 31);

            assert_ok!(Pallet::revoke_permanence(&ALICE));

            let meta = Pallet::get_meta(&ALICE).unwrap();
            assert_eq!(meta.status, AuthorStatus::Probation);
            System::assert_last_event(Event::AuthorStatus { author: ALICE, status: AuthorStatus::Probation }.into());
        })
    }

    #[test]
    fn revoke_permanance_err_author_not_found() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(16);
            Pallet::set_permanence(&ALICE).unwrap();

            let meta = Pallet::get_meta(&ALICE).unwrap();
            assert_eq!(meta.status, AuthorStatus::Active);

            assert_err!(Pallet::revoke_permanence(&BOB), Error::AuthorNotFound);
        })
    }

    #[test]
    fn can_revoke_permanence_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            AuthorsMap::mutate(ALICE, |author| {
                let info = author.as_mut().unwrap();
                let status = &mut info.status;
                let risk_until = &mut info.risk_until;
                *status = AuthorStatus::Active;
                *risk_until = 31;
            });

            System::set_block_number(20);
            assert_ok!(Pallet::can_revoke_permanence(&ALICE));
        })
    }

    #[test]
    fn can_revoke_permanence_err_risk_within_threshold() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            AuthorsMap::mutate(ALICE, |author| {
                let info = author.as_mut().unwrap();
                let status = &mut info.status;
                let risk_until = &mut info.risk_until;
                *status = AuthorStatus::Active;
                *risk_until = 31;
            });

            System::set_block_number(21);
            assert_err!(
                Pallet::can_revoke_permanence(&ALICE),
                Error::RiskWithinThreshold
            );

            System::set_block_number(25);
            assert_err!(
                Pallet::can_revoke_permanence(&ALICE),
                Error::RiskWithinThreshold
            );
        })
    }

    #[test]
    fn can_revoke_permanence_err_author_resigned() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            AuthorsMap::mutate(ALICE, |author| {
                let info = author.as_mut().unwrap();
                let status = &mut info.status;
                *status = AuthorStatus::Resigned;
            });

            System::set_block_number(21);
            assert_err!(Pallet::can_revoke_permanence(&ALICE), Error::AuthorResigned);
        })
    }

    #[test]
    fn can_revoke_permanence_err_author_in_probation() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            AuthorsMap::mutate(ALICE, |author| {
                let info = author.as_mut().unwrap();
                let status = &mut info.status;
                *status = AuthorStatus::Probation;
            });

            System::set_block_number(21);
            assert_err!(
                Pallet::can_revoke_permanence(&ALICE),
                Error::AuthorInProbation
            );
        })
    }

    #[test]
    fn on_set_permance_event_emmisison_success() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            Pallet::on_set_permance(&ALICE);

            let status = AuthorStatus::Active;
            System::assert_last_event(
                Event::AuthorStatus {
                    author: ALICE,
                    status,
                }
                .into(),
            );
        })
    }

    #[test]
    fn on_revoke_permanence_event_emmisison_success() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            Pallet::on_revoke_permanence(&ALICE);

            let status = AuthorStatus::Probation;
            System::assert_last_event(
                Event::AuthorStatus {
                    author: ALICE,
                    status,
                }
                .into(),
            );
        })
    }

    #[test]
    fn on_risk_permanence_event_emmisison_success() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, STANDARD_HOLD).unwrap();
            Pallet::enroll(&ALICE, 100, Fortitude::Force).unwrap();

            Pallet::on_risk_permanence(&ALICE);

            let status = AuthorStatus::Active;
            System::assert_last_event(
                Event::AuthorAtRisk {
                    author: ALICE,
                    status,
                    until: 10,
                }
                .into(),
            );
        })
    }

    #[test]
    fn on_risk_probation_event_emmisison_success() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, STANDARD_HOLD).unwrap();
            Pallet::enroll(&ALICE, 100, Fortitude::Force).unwrap();

            Pallet::on_risk_probation(&ALICE);

            let status = AuthorStatus::Probation;
            System::assert_last_event(
                Event::AuthorAtRisk {
                    author: ALICE,
                    status,
                    until: 10,
                }
                .into(),
            );
        })
    }

    #[test]
    fn on_secure_permanence_event_emmisison_success() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, STANDARD_HOLD).unwrap();
            Pallet::enroll(&ALICE, 100, Fortitude::Force).unwrap();

            Pallet::on_secure_permanence(&ALICE);

            let status = AuthorStatus::Probation;
            System::assert_last_event(
                Event::AuthorAtRisk {
                    author: ALICE,
                    status,
                    until: 10,
                }
                .into(),
            );
        })
    }
}
