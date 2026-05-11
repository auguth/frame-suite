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
// ``````````````````````````````````` OFFENCES ``````````````````````````````````
// ===============================================================================

//! Implements [`OnOffenceHandler`] for [`Pallet`].
//!
//! Maps offenders to authors and schedules penalties via [`PenalizeAuthors`],
//! delegating execution to the author role manager [`Config::RoleAdapter`].

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- Local crate imports ---
use crate::{types::*, weights::*, Config, Internals, Pallet};

// --- FRAME Suite ---
use frame_suite::PenalizeAuthors;

// --- Substrate primitives ---
use sp_runtime::{Vec, Weight};

// --- Substrate staking ---
use sp_staking::offence::{OffenceDetails, OnOffenceHandler};

// ===============================================================================
// ``````````````````````````````` OFFENCE-HANDLER ```````````````````````````````
// ===============================================================================

/// Integration with Substrate's offence reporting system.
///
/// This implementation bridges Substrate's session-level offence detection
/// with the pallet's **author-level penalty scheduling** via the
/// [`PenalizeAuthors`] interface.
///
/// Offences reported by the session pallet are translated into
/// author identifiers and penalty inputs, which are then **scheduled**
/// for enforcement by the pallet's penalty subsystem.
impl<T: Config> OnOffenceHandler<OffenceReporter<T>, Offender<T>, Weight> for Pallet<T>
where
    AuthorOf<T>: From<<T as pallet_session::Config>::ValidatorId>,
{
    /// Handles reported offences for the current session.
    ///
    /// ## Workflow
    /// - Convert session-level offenders into local author identifiers.
    /// - Map each offence to its corresponding penalty fraction.
    /// - Schedule penalties via the pallet's penalty subsystem
    ///   [`PenalizeAuthors::penalize_authors`].
    ///
    /// ## Semantics
    /// - Penalties are **scheduled**, not enforced immediately.
    /// - Final enforcement timing and transformation (e.g. scaling, caps,
    ///   aggregation) are governed by the configured penalty plugin model
    ///   [`Config::PenaltyModel`].
    /// - This handler performs **no additional offence validation** beyond
    ///   the guarantees already provided by Substrate.
    fn on_offence(
        offenders: &[OffenceDetails<OffenceReporter<T>, Offender<T>>],
        slash_fractions: &[PenaltyRatio],
        _session: SessionIndex,
    ) -> Weight {
        // Prepare a list of (author, penalty) pairs.
        let mut offenders_list: Vec<(AuthorOf<T>, PenaltyOf<T>)> = Vec::new();
        
        // Convert each reported offender into a local author identifier
        // and associate it with the corresponding penalty fraction.
        for (i, offender) in offenders.iter().enumerate() {
            let details = offender;
            let slash_fraction = slash_fractions[i];

            let offender_account: AuthorOf<T> = details.offender.0.clone().into();

            offenders_list.push((offender_account, slash_fraction));
        }

        // Schedule penalties for all offending authors via the pallet's
        // penalty subsystem.
        <Internals<T> as PenalizeAuthors<AuthorOf<T>, PenaltyOf<T>>>::penalize_authors(
            offenders_list,
        );

        let offenders_count = offenders.len() as u32;
        <T as Config>::WeightInfo::on_offence(offenders_count)
    }
}

// ===============================================================================
// ```````````````````````````````` OFFENCE TESTS ````````````````````````````````
// ===============================================================================

#[cfg(test)]
mod tests {

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````````` IMPORTS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    // --- Local crate imports ---
    use crate::mock::*;

    // --- FRAME Suite ---
    use frame_suite::roles::*;

    // --- FRAME Support ---
    use frame_support::traits::tokens::Fortitude;

    // --- Substrate staking ---
    use sp_staking::offence::{OffenceDetails, OnOffenceHandler};

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````` OFFENCE HANDLER ```````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn on_offence_success() {
        chain_manager_test_ext().execute_with(|| {
            set_user_balance_and_hold(ALICE, 250, 250).unwrap();
            set_user_balance_and_hold(BOB, 250, 250).unwrap();
            set_user_balance_and_hold(MIKE, 250, 250).unwrap();
            set_user_balance_and_hold(CHARLIE, 250, 250).unwrap();

            RoleAdapter::enroll(&ALICE, 200, Fortitude::Force).unwrap();
            RoleAdapter::enroll(&BOB, 150, Fortitude::Force).unwrap();

            System::set_block_number(16);

            let offenders = [OffenceDetails {
                offender: (ALICE, ALICE),
                reporters: vec![CHARLIE, MIKE],
            }];
            let slash_fraction = [PenaltyRatio::from_percent(5)];

            Pallet::on_offence(&offenders, &slash_fraction, 1);

            // Penalty immediately scheduled for Alice
            let penalties_of_alice = RoleAdapter::get_penalties_of(&ALICE).unwrap();

            let expected_penalties_of_alice = vec![(20, PenaltyRatio::from_percent(5))];
            assert_eq!(penalties_of_alice, expected_penalties_of_alice);
        })
    }
}
