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
// `````````````````````````````` AUTHORS ELECTIONS ``````````````````````````````
// ===============================================================================

//! Provides **concrete election implementations** for author
//! selection using **[`plugin`](frame_suite::plugins)-based election traits**.
//!
//! It binds the generic [`election`](frame_suite::elections) abstractions
//! defined in to pallet-specific storage, configuration, and runtime models.
//!
//! The module implements two distinct election strategies:
//!
//! ## Flat Election
//!
//! - Aggregates all economic exposure of an author (self-collateral and
//!   third-party backing) into a **single influence value**.
//! - Influence is computed via a runtime-configured [`Influence`] plugin model.
//! - Each author contributes exactly one comparable weight into the election.
//!
//! This model favors **total economic commitment**, regardless of its source.
//!
//! ## Fair Election
//!
//! - Preserves **individual backing contributions** from external funders.
//! - Explicitly may include candidate's self-collateral from election weight as
//! one of the backers.
//! - Each backer contributes a distinct weight entry for the author.
//!
//! This model favors **distributed support** and discourages dominance through
//! self-backed influence.
//!
//! ## Architecture
//!
//! Both election modes:
//!
//! - Implement [`InspectWeight`] to expose candidate weights in a
//!   model-appropriate form.
//! - Implement [`ElectionManager`] to:
//!   - prepare election inputs,
//!   - invoke plugin-based election models,
//!   - enforce governance constraints (minimum / maximum elected),
//!   - persist election results,
//!   - and emit lifecycle events.
//!
//! All election computation logic is delegated to **runtime-configured plugins**.
//! This ensures that:
//!
//! - election algorithms can evolve without pallet code changes,
//! - multiple election strategies can coexist safely,
//! - and governance retains control over election semantics.
//!
//! ## Storage Semantics
//!
//! - Election results are stored per block and keyed by the most recent
//!   election round.
//! - Historical elections remain immutable.
//! - Removal operations only affect the latest election state.
//!
//! ## Design Guarantees
//!
//! - No election logic is hardcoded in this module.
//! - Influence and weight calculations are fully externalized.
//! - Strong type safety is preserved across all election paths.
//!
//! This module serves as the **bridge between abstract election traits
//! and pallet-level author governance**.

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- Local crate imports ---
use crate::{
    types::{
        Author, AuthorAsset, BackingElectionWeight, ElectViaBacking, ElectViaInfluence,
        ElectedAuthors,
    },
    Config, Elected, Error, Event, FairElection, FlatElection, ForceMaxElected, MaxElected,
    MinElected, Pallet, RecentElectedOn,
};

// --- FRAME Suite ---
use frame_suite::{
    elections::{ElectionManager, Influence, InspectWeight},
    roles::{CompensateRoles, FundRoles},
};

// --- Substrate primitives ---
use sp_core::Get;
use sp_runtime::{traits::Zero, DispatchError, DispatchResult, Vec};

// --- Substrate std (no_std helpers) ---
use sp_std::vec;

// ===============================================================================
// ```````````````````````````````` FLAT-ELECTION ````````````````````````````````
// ===============================================================================

/// Implementation of the [`Influence`] trait for [`FlatElection`].
///
/// This binds the [`FlatElection`] election system to a **concrete
/// influence computation** using [`Config::InfluenceModel`]
///
/// ## Influence Input
///
/// - [`AuthorAsset`]: The raw input type used to compute influence.
/// - Typically represents an aggregated backing asset associated
/// with the author ([`Author`]).
impl<T: Config> Influence<AuthorAsset<T>> for FlatElection<T> {
    /// The resulting influence type, as defined in the
    /// runtime configuration.
    type Influence = T::Influence;

    /// The plugin context used for influence computation, providing
    /// runtime parameters, thresholds, or local configuration needed
    /// by the plugin model.
    type InfluenceContext = T::InfluenceContext;

    /// The plugin model used to perform the computation, implementing
    /// the logic to convert [`AuthorAsset`] into `Self::Influence`.
    type InfluenceModel = T::InfluenceModel;
}

/// Implementation of the [`InspectWeight`] trait for [`FlatElection`].
///
/// This provides a way to **inspect the computed weight** of an [`Author`]
/// in terms of influence, leveraging the generic influence computation
/// defined in the runtime.
///
/// ## Author Weight
///
/// - Returns the weight of a author as a `Vec<Influence>`.
/// - Although `Influence` is a singular value, it is wrapped in a vector to
///   satisfy the generic input requirements of [`ElectionManager`] for swappable
///   [`FairElection`] and [`FlatElection`].
/// - Each element (typically only one) represents a computed influence derived
///   from the author's backing asset.
impl<T: Config> InspectWeight<Author<T>, Vec<T::Influence>> for FlatElection<T> {
    /// Returns the influence weight of an author wrapped in a vector.
    ///
    /// ## Behavior
    /// 1. Fetches the total backing asset of the author using [`CompensateRoles::get_hold`].
    /// 2. Computes the author's influence using the [`Influence`] implementation for [`FlatElection`].
    /// 3. Wraps the result in a `Vec` and returns it.
    ///
    /// ## Errors
    /// - Returns a [`DispatchError`] if fails.
    fn weight_of(who: &Author<T>) -> Result<Vec<T::Influence>, DispatchError> {
        // Fetch the backing asset (total hold) of the author (includes collateral + funding)
        let hold = Pallet::<T>::get_hold(who)?;
        // Compute influence from the asset
        let influence = <Self as Influence<AuthorAsset<T>>>::influence(hold);
        // return as a vector (`ElectionManager` trait's input param compatible)
        Ok(vec![influence])
    }
}

/// Implementation of the [`ElectionManager`] trait for [`FlatElection`].
///
/// This binds the [`FlatElection`] system to a **concrete election computation**
/// using influence-based metrics, leveraging runtime-configured plugin models and contexts.
///
/// - Election weights are computed from [`Influence`] values for [`Author`].
/// - Input type to the election plugin is [`ElectViaInfluence`] (candidates with their
/// backing influences).
/// - Output type is [`ElectedAuthors`] (a collection of elected candidates).
impl<T: Config> ElectionManager<Author<T>> for FlatElection<T> {
    /// Election weight type: corresponds to the singular influence of an author.
    type ElectionWeight = T::Influence;

    /// Collection type holding weights for an author.
    ///
    /// Although it is a vector, it typically contains only a single influence value.
    type ElectionWeightOf = Vec<Self::ElectionWeight>;

    /// Input type for election computation: authors paired with influence weights.
    type Params = ElectViaInfluence<T>;

    /// Output type representing elected authors.
    type Elected = ElectedAuthors<T>;

    /// Plugin context providing runtime configuration for the election model.
    type ElectionContext = T::FlatElectionContext;

    /// Plugin model implementing the election algorithm.
    type ElectionModel = T::FlatElectionModel;

    /// Retrieve the currently elected candidates.
    ///
    /// Fetches the authors elected in the **most recent election round**,
    /// using [`RecentElectedOn`] to determine the latest block where
    /// results were stored.
    ///
    /// ## Returns
    /// - `Ok(Elected)` - A collection of elected authors.
    /// - `None` - if none are elected
    fn reveal() -> Option<Self::Elected> {
        // Retrieve the most recent election block number.
        let block = RecentElectedOn::<T>::get();

        // Iterate over all elected authors stored under that block.
        let iter = Elected::<T>::iter_prefix((block,));

        // Prepare a collection for the converted elected authors.
        let mut elects_converted: Self::Elected = Default::default();

        // Collect all elected authors from storage.
        for (author, _) in iter {
            elects_converted.push(author.into());
        }

        // Return an error if no elected candidates were found.
        if elects_converted.is_empty() {
            return None;
        }

        Some(elects_converted)
    }

    /// Optimistically removes a candidate from the **most recent elected pool**.
    ///
    /// Directly updates storage by deleting the `(block, author)` entry
    /// from [`Elected`] for the latest election block.
    ///
    /// Does not retroactively remove authors from historical elections.
    fn remove(who: &Author<T>) {
        let block = RecentElectedOn::<T>::get();
        Elected::<T>::remove((block, who));
    }

    /// Search if the given candidate is an elected in the recent election.
    ///
    /// `DispatchError` otherwise
    fn is_candidate(who: &Author<T>) -> DispatchResult {
        let all = Self::reveal();
        if let Some(elects) = all {
            for elect in elects {
                if elect == *who {
                    return Ok(());
                }
            }
        }
        Err(Error::<T>::AuthorNotElected.into())
    }

    /// Persist the election results into storage.
    ///
    /// Stores the newly elected authors under the current block number,
    /// updating [`Elected`] and [`RecentElectedOn`].
    ///
    /// ## Behavior
    /// - Ensures the number of elected candidates meets the minimum requirement.
    /// - Optionally truncates to the maximum limit if [`ForceMaxElected`] is enabled.
    /// - Each elected author is stored under `(current_block, author)`.
    ///
    /// ## Errors
    /// - Returns a [`DispatchError`] if fails.
    fn store(elects: &Self::Elected) -> DispatchResult {
        let min_elect = MinElected::<T>::get();
        debug_assert!(
            !min_elect.is_zero(),
            "`MinElected` must be greater than zero"
        );
        debug_assert!(
            min_elect <= MaxElected::<T>::get(),
            "`MinElected` must be lesser than or equal to `MaxElected`"
        );
        // Enforce the minimum elected candidate constraint.
        if elects.len() < (min_elect as usize) {
            return Err(Error::<T>::MinElectedNotReached.into());
        }

        // Get the current block number to use as the election key.
        let block = frame_system::Pallet::<T>::block_number();

        // Handle the maximum elected constraint.
        match ForceMaxElected::<T>::get() {
            // If forced, truncate to the configured maximum.
            true => {
                let max = MaxElected::<T>::get();
                debug_assert!(
                    max >= MinElected::<T>::get(),
                    "`MaxElected` must be greater than or equal to `MinElected`"
                );
                let mut final_result = elects.clone();
                final_result.truncate(max as usize);
                for author in final_result {
                    Elected::<T>::insert((block, author), ());
                }
            }
            // Otherwise, store all elected authors directly.
            false => {
                for author in elects.iter() {
                    Elected::<T>::insert((block, author), ());
                }
            }
        }

        // Record the block number of this election round.
        RecentElectedOn::<T>::put(block);

        Ok(())
    }

    /// Check if the election can be prepared with the given authors as candidates.
    ///
    /// Ensures that the provided candidate set meets the configured minimum
    /// ([`MinElected`]) before initiating election computation.
    fn can_prepare(from: &Self::Params) -> DispatchResult {
        let min_elect = MinElected::<T>::get();
        debug_assert!(
            !min_elect.is_zero(),
            "`MinElected` must be greater than zero"
        );
        debug_assert!(
            min_elect <= MaxElected::<T>::get(),
            "`MinElected` must be lesser than or equal to `MaxElected`"
        );
        if from.len() < min_elect as usize {
            return Err(Error::<T>::InadequateCandidatesToElect.into());
        }
        Ok(())
    }

    /// Hook invoked after a successful election process.
    fn on_prepare_success(elects: &Self::Elected) {
        if T::EmitEvents::get() {
            Pallet::<T>::deposit_event(Event::<T>::ElectionPrepared {
                elects: elects.clone(),
            });
        }
    }

    /// Hook invoked when an election process fails.
    fn on_prepare_fail(error: DispatchError) {
        if T::EmitEvents::get() {
            Pallet::<T>::deposit_event(Event::<T>::ElectionFailed { error });
        }
    }
}

// ===============================================================================
// ```````````````````````````````` FAIR-ELECTION ````````````````````````````````
// ===============================================================================

/// Implementation of the [`InspectWeight`] trait for [`FairElection`].
///
/// This provides a way to **inspect the backing weights** of an [`Author`]
/// in terms of their individual contributions from backers, leveraging the stored
/// backing information in the pallet.
///
/// ## Author Weight
///
/// - Returns the weight of an author as a `Vec<BackingElectionWeight>`.
///   Each element represents the backing contribution of an individual backer i.e.,
///   an external funder.
/// - Wrapping the contributions in a vector satisfies the generic input requirements
///   of [`ElectionManager`] for both [`FairElection`] and [`FlatElection`].
impl<T: Config> InspectWeight<Author<T>, Vec<BackingElectionWeight<T>>> for FairElection<T> {
    /// Returns the backing weights of an author wrapped in a vector.
    ///
    /// ## Behavior
    /// 1. Fetches the list of backers for the author using [`FundRoles::backers_of`].
    /// 2. Returns the backers' contributions as a vector.
    ///
    /// ## Errors
    /// - Returns a [`DispatchError`] if fails.
    fn weight_of(who: &Author<T>) -> Result<Vec<BackingElectionWeight<T>>, DispatchError> {
        // Fetch the list of backers for the author
        let backers = Pallet::<T>::backers_of(who)?;
        Ok(backers)
    }
}

/// Implementation of the [`ElectionManager`] trait for [`FairElection`].
///
/// This binds the [`FairElection`] system to a **concrete election computation**
/// using backing-based metrics, leveraging runtime-configured plugin models and contexts.
///
/// - Election weights are computed from [`BackingElectionWeight`] values for [`Author`].
/// - Input type to the election plugin is [`ElectViaBacking`] (candidates with their
///   backing contributions).
/// - Output type is [`ElectedAuthors`] (a collection of elected candidates).
impl<T: Config> ElectionManager<Author<T>> for FairElection<T> {
    /// Election weight type: corresponds to a singular backing contribution to an author.
    type ElectionWeight = BackingElectionWeight<T>;

    /// Collection type holding all weights (backing contributions) of an author.
    ///
    /// Each author may have multiple backers; this vector represents all backing weights.
    type ElectionWeightOf = Vec<Self::ElectionWeight>;

    /// Input type for election computation: authors paired with their backing weights.
    type Params = ElectViaBacking<T>;

    /// Output type representing elected authors.
    type Elected = ElectedAuthors<T>;

    /// Plugin context providing runtime configuration for the election model.
    type ElectionContext = T::FairElectionContext;

    /// Plugin model implementing the election algorithm.
    type ElectionModel = T::FairElectionModel;

    //----- Redudant method implementations similar to `FlatElection` ---------

    /// Retrieve the currently elected candidates.
    ///
    /// Fetches the authors elected in the **most recent election round**,
    /// using [`RecentElectedOn`] to determine the latest block where
    /// results were stored.
    ///
    /// ## Returns
    /// - `Ok(Elected)` - A collection of elected authors.
    /// - `None` otherwise.
    fn reveal() -> Option<Self::Elected> {
        // Retrieve the most recent election block number.
        let block = RecentElectedOn::<T>::get();

        // Iterate over all elected authors stored under that block.
        let iter = Elected::<T>::iter_prefix((block,));

        // Prepare a collection for the converted elected authors.
        let mut elects_converted: Self::Elected = Default::default();

        // Collect all elected authors from storage.
        for (author, _) in iter {
            elects_converted.push(author.into());
        }

        // Return an error if no elected candidates were found.
        if elects_converted.is_empty() {
            return None;
        }

        Some(elects_converted)
    }

    /// Optimistically remove a candidate from the **most recent elected pool**.
    ///
    /// Directly updates storage by deleting the `(block, author)` entry
    /// from [`Elected`] for the latest election block.
    ///
    /// Does not retroactively remove authors from historical elections.
    fn remove(who: &Author<T>) {
        let block = RecentElectedOn::<T>::get();
        Elected::<T>::remove((block, who));
    }

    /// Search if the given candidate is an elected in the recent election.
    ///
    /// `DispatchError` otherwise
    fn is_candidate(who: &Author<T>) -> DispatchResult {
        let all = Self::reveal();
        if let Some(elects) = all {
            for elect in elects {
                if elect == *who {
                    return Ok(());
                }
            }
        }
        Err(Error::<T>::AuthorNotElected.into())
    }

    /// Persist the election results into storage.
    ///
    /// Stores the newly elected authors under the current block number,
    /// updating [`Elected`] and [`RecentElectedOn`].
    ///
    /// ## Behavior
    /// - Ensures the number of elected candidates meets the minimum requirement.
    /// - Optionally truncates to the maximum limit if [`ForceMaxElected`] is enabled.
    /// - Each elected author is stored under `(current_block, author)`.
    ///
    /// ## Errors
    /// - Returns a [`DispatchError`] if fails.
    fn store(elects: &Self::Elected) -> DispatchResult {
        let min_elect = MinElected::<T>::get();
        debug_assert!(
            !min_elect.is_zero(),
            "`MinElected` must be greater than zero"
        );
        debug_assert!(
            min_elect <= MaxElected::<T>::get(),
            "`MinElected` must be lesser than or equal to `MaxElected`"
        );
        // Enforce the minimum elected candidate constraint.
        if elects.len() < (min_elect as usize) {
            return Err(Error::<T>::MinElectedNotReached.into());
        }

        // Get the current block number to use as the election key.
        let block = frame_system::Pallet::<T>::block_number();

        // Handle the maximum elected constraint.
        match ForceMaxElected::<T>::get() {
            // If forced, truncate to the configured maximum.
            true => {
                let max = MaxElected::<T>::get();
                debug_assert!(
                    max >= MinElected::<T>::get(),
                    "`MaxElected` must be greater than or equal to `MinElected`"
                );
                let mut final_result = elects.clone();
                final_result.truncate(max as usize);
                for author in final_result {
                    Elected::<T>::insert((block, author), ());
                }
            }
            // Otherwise, store all elected authors directly.
            false => {
                for author in elects.iter() {
                    Elected::<T>::insert((block, author), ());
                }
            }
        }

        // Record the block number of this election round.
        RecentElectedOn::<T>::put(block);

        Ok(())
    }

    /// Check if the election can be prepared with the given authors as candidates.
    ///
    /// Ensures that the provided candidate set meets the configured minimum
    /// (`MinElected`) before initiating election computation.
    fn can_prepare(from: &Self::Params) -> DispatchResult {
        let min_elect = MinElected::<T>::get();
        debug_assert!(
            !min_elect.is_zero(),
            "`MinElected` must be greater than zero"
        );
        debug_assert!(
            min_elect <= MaxElected::<T>::get(),
            "`MinElected` must be lesser than or equal to `MaxElected`"
        );
        if from.len() < min_elect as usize {
            return Err(Error::<T>::InadequateCandidatesToElect.into());
        }
        Ok(())
    }

    /// Hook invoked after a successful election process.
    fn on_prepare_success(elects: &Self::Elected) {
        if T::EmitEvents::get() {
            Pallet::<T>::deposit_event(Event::<T>::ElectionPrepared {
                elects: elects.clone(),
            });
        }
    }
    /// Hook invoked when an election process fails.
    fn on_prepare_fail(error: DispatchError) {
        if T::EmitEvents::get() {
            Pallet::<T>::deposit_event(Event::<T>::ElectionFailed { error });
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
    use crate::types::Funder;
    use crate::{mock::*, Elected, RecentElectedOn};

    // --- FRAME Suite ---
    use frame_suite::{roles::*, ElectionManager, InspectWeight};

    use frame_support::{assert_err, assert_ok};
    // --- FRAME Support ---
    use frame_support::traits::tokens::{Fortitude, Precision};
    use sp_runtime::AccountId32;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````` FLAT-ELECTION ````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn weight_of_success_for_flat_election() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(6);
            // ALICE enrolls with a collateral of 100 units
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

            let influence =
                <FlatElection as InspectWeight<AccountId32, Vec<u64>>>::weight_of(&ALICE).unwrap();

            assert_eq!(influence, vec![225]);
        })
    }

    #[test]
    fn reveal_success_for_flat_election() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            RecentElectedOn::<Test>::put(10);
            Elected::<Test>::insert((10, ALICE), ());
            Elected::<Test>::insert((10, BOB), ());
            Elected::<Test>::insert((10, MIKE), ());
            Elected::<Test>::insert((10, NIX), ());
            Elected::<Test>::insert((10, ALAN), ());
            Elected::<Test>::insert((10, AMY), ());

            let mut actual_elected =
                <FlatElection as ElectionManager<AccountId32>>::reveal().unwrap();

            let mut expected_elected = vec![ALICE, BOB, MIKE, NIX, ALAN, AMY];
            actual_elected.sort();
            expected_elected.sort();
            assert_eq!(actual_elected, expected_elected);
        })
    }

    #[test]
    fn reveal_returns_none_for_flat_election() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(450);
            RecentElectedOn::<Test>::put(450);
            Elected::<Test>::insert((450, ALICE), ());
            Elected::<Test>::insert((450, BOB), ());
            Elected::<Test>::insert((450, MIKE), ());
            Elected::<Test>::insert((450, NIX), ());
            Elected::<Test>::insert((450, ALAN), ());
            Elected::<Test>::insert((450, AMY), ());

            System::set_block_number(900);
            RecentElectedOn::<Test>::put(900);
            assert!(<FlatElection as ElectionManager<AccountId32>>::reveal().is_none());
        })
    }

    #[test]
    fn remove_success_for_flat_election() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            RecentElectedOn::<Test>::put(10);
            Elected::<Test>::insert((10, ALICE), ());
            Elected::<Test>::insert((10, BOB), ());
            Elected::<Test>::insert((10, MIKE), ());
            Elected::<Test>::insert((10, NIX), ());
            Elected::<Test>::insert((10, ALAN), ());
            Elected::<Test>::insert((10, AMY), ());

            let mut actual_elected =
                <FlatElection as ElectionManager<AccountId32>>::reveal().unwrap();
            let mut expected_elected = vec![ALICE, BOB, MIKE, NIX, ALAN, AMY];
            actual_elected.sort();
            expected_elected.sort();
            assert_eq!(actual_elected, expected_elected);

            <FlatElection as ElectionManager<AccountId32>>::remove(&NIX);

            let mut actual_elected =
                <FlatElection as ElectionManager<AccountId32>>::reveal().unwrap();
            let mut expected_elected = vec![ALICE, BOB, MIKE, ALAN, AMY];
            actual_elected.sort();
            expected_elected.sort();
            assert_eq!(actual_elected, expected_elected);

            <FlatElection as ElectionManager<AccountId32>>::remove(&AMY);

            let mut actual_elected =
                <FlatElection as ElectionManager<AccountId32>>::reveal().unwrap();
            let mut expected_elected = vec![ALICE, BOB, MIKE, ALAN];
            actual_elected.sort();
            expected_elected.sort();
            assert_eq!(actual_elected, expected_elected);
        })
    }

    #[test]
    fn is_candidate_success_for_flat_election() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            RecentElectedOn::<Test>::put(10);
            Elected::<Test>::insert((10, ALICE), ());
            Elected::<Test>::insert((10, BOB), ());
            Elected::<Test>::insert((10, MIKE), ());
            Elected::<Test>::insert((10, ALAN), ());
            Elected::<Test>::insert((10, AMY), ());

            assert_ok!(<FlatElection as ElectionManager<AccountId32>>::is_candidate(&ALICE));
            assert_err!(
                <FlatElection as ElectionManager<AccountId32>>::is_candidate(&NIX),
                Error::AuthorNotElected
            );
        })
    }

    #[test]
    fn store_success_for_flat_election() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(25);
            let mut elects = vec![ALICE, ALAN, NIX, AMY, MIKE, BOB];
            assert!(<FlatElection as ElectionManager<AccountId32>>::reveal().is_none());
            assert_ok!(<FlatElection as ElectionManager<AccountId32>>::store(
                &elects
            ));

            let elected = <FlatElection as ElectionManager<AccountId32>>::reveal();
            assert!(elected.is_some());
            let mut elected = elected.unwrap();
            elects.sort(); 
            elected.sort();
            assert_eq!(elects, elected);
            assert_eq!(RecentElectedOn::<Test>::get(), 25);
        })
    }

    #[test]
    fn store_err_min_elected_not_reached_for_flat_election() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            let elects = vec![ALICE, ALAN, NIX, AMY, MIKE];
            // Since, min_elected is set to 6
            assert_err!(
                <FlatElection as ElectionManager<AccountId32>>::store(&elects),
                Error::MinElectedNotReached
            );
        })
    }

    #[test]
    fn on_prepare_success_emits_event_for_flat_election() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            let elects = vec![ALICE, ALAN, NIX, AMY, MIKE, BOB];
            <FlatElection as ElectionManager<AccountId32>>::on_prepare_success(&elects);

            System::assert_last_event(Event::ElectionPrepared { elects }.into());
        })
    }

    #[test]
    fn on_prepare_fail_emits_event_for_flat_election() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            let error = Error::MinElectedNotReached.into();
            <FlatElection as ElectionManager<AccountId32>>::on_prepare_fail(error);

            System::assert_last_event(Event::ElectionFailed { error: error }.into());
        })
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````` FAIR-ELECTION ````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn weight_of_success_for_fair_election() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, LARGE_VALUE, LARGE_VALUE).unwrap();

            Pallet::enroll(&ALICE, LARGE_VALUE, Fortitude::Force).unwrap();

            Pallet::enroll(&BOB, STANDARD_VALUE, Fortitude::Force).unwrap();

            Pallet::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                STANDARD_VALUE,
                Precision::BestEffort,
                Fortitude::Force,
            )
            .unwrap();

            Pallet::fund(
                &BOB,
                &Funder::Direct(MIKE),
                LARGE_VALUE,
                Precision::BestEffort,
                Fortitude::Force,
            )
            .unwrap();

            Pallet::fund(
                &ALICE,
                &Funder::Direct(ALAN),
                SMALL_VALUE,
                Precision::BestEffort,
                Fortitude::Force,
            )
            .unwrap();

            let alice_weight = FairElection::weight_of(&ALICE).unwrap();

            let bob_weight = FairElection::weight_of(&BOB).unwrap();

            let expected_alice_weight =
                vec![(Funder::Direct(ALAN), 25), (Funder::Direct(CHARLIE), 50)];
            let expected_bob_weight = vec![(Funder::Direct(MIKE), 100)];

            assert_eq!(alice_weight, expected_alice_weight);
            assert_eq!(bob_weight, expected_bob_weight);
        })
    }

    #[test]
    fn reveal_success_for_fair_election() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            RecentElectedOn::<Test>::put(10);
            Elected::<Test>::insert((10, ALICE), ());
            Elected::<Test>::insert((10, BOB), ());
            Elected::<Test>::insert((10, MIKE), ());
            Elected::<Test>::insert((10, NIX), ());
            Elected::<Test>::insert((10, ALAN), ());
            Elected::<Test>::insert((10, AMY), ());

            let mut actual_elected =
                <FairElection as ElectionManager<AccountId32>>::reveal().unwrap();

            let mut expected_elected = vec![ALICE, BOB, MIKE, NIX, ALAN, AMY];
            actual_elected.sort(); 
            expected_elected.sort();
            assert_eq!(actual_elected, expected_elected);
        })
    }

    #[test]
    fn reveal_returns_none_for_fair_election() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(450);
            RecentElectedOn::<Test>::put(450);
            Elected::<Test>::insert((450, ALICE), ());
            Elected::<Test>::insert((450, BOB), ());
            Elected::<Test>::insert((450, MIKE), ());
            Elected::<Test>::insert((450, NIX), ());
            Elected::<Test>::insert((450, ALAN), ());
            Elected::<Test>::insert((450, AMY), ());

            System::set_block_number(900);
            RecentElectedOn::<Test>::put(900);
            assert!(<FairElection as ElectionManager<AccountId32>>::reveal().is_none());
        })
    }

    #[test]
    fn remove_success_for_fair_election() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            RecentElectedOn::<Test>::put(10);
            Elected::<Test>::insert((10, ALICE), ());
            Elected::<Test>::insert((10, BOB), ());
            Elected::<Test>::insert((10, MIKE), ());
            Elected::<Test>::insert((10, NIX), ());
            Elected::<Test>::insert((10, ALAN), ());
            Elected::<Test>::insert((10, AMY), ());

            let mut actual_elected =
                <FairElection as ElectionManager<AccountId32>>::reveal().unwrap();
            let mut expected_elected = vec![ALICE, BOB, MIKE, NIX, ALAN, AMY];
            actual_elected.sort();
            expected_elected.sort();
            assert_eq!(actual_elected, expected_elected);

            <FairElection as ElectionManager<AccountId32>>::remove(&NIX);

            let mut actual_elected =
                <FairElection as ElectionManager<AccountId32>>::reveal().unwrap();
            let mut expected_elected = vec![ALICE, BOB, MIKE, ALAN, AMY];
            actual_elected.sort();
            expected_elected.sort();
            assert_eq!(actual_elected, expected_elected);

            <FairElection as ElectionManager<AccountId32>>::remove(&AMY);

            let mut actual_elected =
                <FairElection as ElectionManager<AccountId32>>::reveal().unwrap();
            let mut expected_elected = vec![ALICE, BOB, MIKE, ALAN];
            actual_elected.sort();
            expected_elected.sort();
            assert_eq!(actual_elected, expected_elected);
        })
    }

    #[test]
    fn is_candidate_success_for_fair_election() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            RecentElectedOn::<Test>::put(10);
            Elected::<Test>::insert((10, ALICE), ());
            Elected::<Test>::insert((10, BOB), ());
            Elected::<Test>::insert((10, MIKE), ());
            Elected::<Test>::insert((10, ALAN), ());
            Elected::<Test>::insert((10, AMY), ());

            assert_ok!(<FairElection as ElectionManager<AccountId32>>::is_candidate(&ALICE));
            assert_err!(
                <FairElection as ElectionManager<AccountId32>>::is_candidate(&NIX),
                Error::AuthorNotElected
            );
        })
    }

    #[test]
    fn store_success_for_fair_election() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(25);
            let mut elects = vec![ALICE, ALAN, NIX, AMY, MIKE, BOB];
            assert!(<FairElection as ElectionManager<AccountId32>>::reveal().is_none());
            assert_ok!(<FairElection as ElectionManager<AccountId32>>::store(
                &elects
            ));

            let elected = <FairElection as ElectionManager<AccountId32>>::reveal();
            assert!(elected.is_some());
            let mut elected = elected.unwrap();
            elects.sort();
            elected.sort();
            assert_eq!(elects, elected);
            assert_eq!(RecentElectedOn::<Test>::get(), 25);
        })
    }

    #[test]
    fn store_err_min_elected_not_reached_for_fair_election() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            let elects = vec![ALICE, ALAN, NIX, AMY, MIKE];
            // Since, min_elected is set to 6
            assert_err!(
                <FairElection as ElectionManager<AccountId32>>::store(&elects),
                Error::MinElectedNotReached
            );
        })
    }

    #[test]
    fn on_prepare_success_emits_event_for_fair_election() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            let elects = vec![ALICE, ALAN, NIX, AMY, MIKE, BOB];
            <FairElection as ElectionManager<AccountId32>>::on_prepare_success(&elects);

            System::assert_last_event(Event::ElectionPrepared { elects }.into());
        })
    }

    #[test]
    fn on_prepare_fail_emits_event_for_fair_election() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            let error = Error::MinElectedNotReached.into();
            <FairElection as ElectionManager<AccountId32>>::on_prepare_fail(error);

            System::assert_last_event(Event::ElectionFailed { error: error }.into());
        })
    }
}
