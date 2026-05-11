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
// ``````````````````````````````` AUTHOR ACTIVITY ```````````````````````````````
// ===============================================================================

//! Implements [`RoleActivity`] for [`Pallet`].
//!
//! Derives author activity from session state and election lifecycle
//! to determine whether an author is idle (not validating) or active.

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- Core / Std ---
use core::marker::PhantomData;

// --- Local crate imports ---
use crate::{
    types::*, AuthorAffidavits, Config, CurrentSession,
    Error, Internals, Pallet,
};

// --- Scale-codec crates ---
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

// --- FRAME Suite ---
use frame_suite::{blockchain::*, roles::RoleActivity};

// --- Substrate primitives ---
use sp_runtime::{
    traits::{Convert, One},
    DispatchError,
};

// ===============================================================================
// ``````````````````````````````````` STRUCTS ```````````````````````````````````
// ===============================================================================

/// Represents the **current blocking duty** being performed by an author.
///
/// This enum is used as the activity context for [`RoleActivity`], indicating
/// why an author is considered *active* and therefore temporarily unable to
/// perform certain operations (e.g. resigning or withdrawing collateral).
///
/// Each variant must map to a **user-facing, actionable [`DispatchError`]**
/// explaining the ongoing duty and how or when it can be exited.
///
/// ## Invariants
/// - An author may be blocked by **at most one** activity at a time.
/// - Activity states are **derived**, not persisted.
///
/// ## Design Notes
/// - Activity is inferred from session state, affidavits, and election results.
/// - No explicit activity storage is maintained.
/// - This enum is strictly descriptive and has no side effects.
#[derive(
    Encode, Decode, Clone, Copy, Eq, PartialEq, TypeInfo, MaxEncodedLen, DecodeWithMemTracking,
)]
#[scale_info(skip_type_params(T))]
pub enum AuthorActivity<T: Config> {
    /// The author is actively validating in the current session.
    SessionValidator,

    /// The author has submitted an affidavit and is participating
    /// in the ongoing election process.
    ElectionCandidate,

    /// The author has won the election and is waiting to enter
    /// the next validation session.
    ElectionWinner,

    /// Internal fallback variant used when activity cannot be
    /// determined conclusively.
    Indeterminate(PhantomData<T>),
}

impl<T: Config> Into<DispatchError> for AuthorActivity<T> {
    fn into(self) -> DispatchError {
        match self {
            AuthorActivity::SessionValidator => Error::<T>::ActivelyValidating.into(),
            AuthorActivity::ElectionCandidate => Error::<T>::ActivelyContestingElection.into(),
            AuthorActivity::ElectionWinner => Error::<T>::ActivelyWarmingForValidation.into(),
            AuthorActivity::Indeterminate(_) => Error::<T>::CannotDetermineAuthorActiveDuty.into(),
        }
    }
}

impl<T: Config> core::fmt::Debug for AuthorActivity<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::SessionValidator => f.write_str("SessionValidator"),
            Self::ElectionCandidate => f.write_str("ElectionCandidate"),
            Self::ElectionWinner => f.write_str("ElectionWinner"),
            Self::Indeterminate(_) => f.write_str("Indeterminate"),
        }
    }
}

// ===============================================================================
// ```````````````````````````````` ROLE ACTIVITY ````````````````````````````````
// ===============================================================================

/// Implementation of the [`RoleActivity`] trait for authors.
///
/// This implementation determines whether an author is currently *idle* / *active*
/// or *blocked* by an active protocol duty.
///
/// ## Design Notes
/// - Activity is computed dynamically on each invocation.
/// - No state is cached or persisted.
/// - Time gating is derived from session timing and affidavit windows.
///
/// ## Caller Responsibility
/// - Callers must handle the returned `AuthorActivity` and propagate
///   its associated [`DispatchError`] to the user for exit solutions.
impl<T: Config> RoleActivity<AuthorOf<T>, AuthorTimeStampOf<T>> for Pallet<T> {
    /// Represents the duty, the author is currently performing.
    type Activity = AuthorActivity<T>;

    /// Determines whether an author is currently idle or blocked by an active duty.
    ///
    /// ## Semantics
    /// - Returns `Ok(())` if the author is idle
    /// - Returns `Err(AuthorActivity)` describing the blocking duty
    fn is_idle(who: &AuthorOf<T>) -> Result<(), AuthorActivity<T>> {
        // If the author cannot be mapped to a session validator ID,
        // they are not actively validating.
        let Some(validator) =
            <Pallet<T> as Convert<AuthorOf<T>, Option<SessionId<T>>>>::convert(who.clone())
        else {
            return Ok(());
        };

        // Block if the author is an active validator in the current session.
        if pallet_session::Pallet::<T>::validators().contains(&validator) {
            return Err(AuthorActivity::<T>::SessionValidator);
        }

        let current_session = CurrentSession::<T>::get();
        let next_session = current_session.saturating_add(One::one());

        // Compute affidavit submission window boundaries.
        let Ok(aff_window) = Pallet::<T>::compute_affidavit_window() else {
            return Err(AuthorActivity::Indeterminate(PhantomData));
        };
        let start_affidavit = aff_window.start;
        let end_affidavit = aff_window.end;

        let current_block = frame_system::Pallet::<T>::block_number();

        // Before affidavit submission begins, non-validating authors are idle.
        if current_block < start_affidavit {
            return Ok(());
        }

        // During affidavit submission, block authors who have
        // submitted an affidavit and are participating in the election.
        if current_block < end_affidavit {
            if AuthorAffidavits::<T>::contains_key((next_session, who)) {
                return Err(AuthorActivity::ElectionCandidate);
            }
        }

        // After the election window, block authors who were elected
        // and are awaiting the next validation session.
        if current_block > end_affidavit {
            if let Some(elected) =
                <Internals<T> as ElectAuthors<AuthorOf<T>, ElectionVia<T>>>::reveal()
            {
                for elect in elected.into_iter() {
                    if *who == elect {
                        return Err(AuthorActivity::ElectionWinner);
                    }
                }
            }
        }

        Ok(())
    }
}