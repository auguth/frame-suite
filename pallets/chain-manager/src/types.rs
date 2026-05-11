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
// ````````````````````````````` CHAIN MANAGER TYPES `````````````````````````````
// ===============================================================================

//! **Core types and aliases for the Chain Manager system.**
//!
//! This module primarily defines **type aliases**, **public structs**, and
//! **runtime-specialized unsigned payloads** used across
//! [`pallet_chain_manager`](crate).
//!
//! The Chain Manager relies on external trait adapters such as:
//! - [`Config::RoleAdapter`]
//! - [`Config::ElectionAdapter`]
//! - [`Config::Asset`] (fungible-adapter)
//! - [`Config::PointsAdapter`]
//!
//! These abstractions delegate core logic to other pallets/framework layers,
//! while the types in this module provide a **unified, runtime-bound view**
//! and are used in the pallet's **public APIs**.
//!
//! Raw unsigned payload types are defined in [`crate::crypto`], but it is
//! recommended to use the aliases here to ensure correct runtime binding
//! to offchain signing types.
//!
//! ## Example
//!
//! ```ignore
//! use pallet_chain_manager::types::ElectionPayloadOf;
//!
//! let payload = ElectionPayloadOf::<T> { ... };
//! SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(
//!     Call::elect { payload }.into()
//! );
//! ```

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- Local crate imports ---
use crate::{crypto::*, Config};

// --- Scale-codec crates ---
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

// --- FRAME Suite ---
use frame_suite::{elections::*, roles::*, routines::Moment};

// --- FRAME Support ---
use frame_support::{
    pallet_prelude::TransactionPriority, traits::fungible::Inspect, RuntimeDebugNoBound,
};

// --- FRAME System ---
use frame_system::{offchain::SigningTypes, pallet_prelude::BlockNumberFor};

// --- Substrate primitives ---
use sp_core::RuntimeDebug;
use sp_runtime::{Perbill, Permill, Vec};

// ===============================================================================
// ``````````````````````````````````` ALIASES ```````````````````````````````````
// ===============================================================================

/// The `AccountId` type representing an author in this runtime.
///
/// Acts as the primary key for tracking points, rewards, penalties, and election data.
pub type AuthorOf<T> = <T as frame_system::pallet::Config>::AccountId;

/// The asset type associated with an author-role.
///
/// Used for funding, collateral, rewarding or penalizing authors and backers.
pub type AssetOf<T> = <<T as Config>::RoleAdapter as RoleManager<AuthorOf<T>>>::Asset;

/// Timestamp associated with an author role for various operations
/// such as deffered rewards and penalities
pub type AuthorTimeStampOf<T> = <<T as Config>::RoleAdapter as RoleManager<AuthorOf<T>>>::TimeStamp;

/// Type representing an entity or account backing an author, as managed by [`FundRoles`].
///
/// - Typically used to track external funding, sponsorship, or support for authors.
/// - Allows pallets to query or interact with the backing source of an author.
pub type BackerOf<T> = <<T as Config>::RoleAdapter as FundRoles<AuthorOf<T>>>::Backer;

/// The concrete fungible balance type of the asset associated with an author.
///
/// Represents the fungible units that can be rewarded, or penalized.
///
/// This is ensured to be [`AssetOf`] where `AssetOf == ActualAsset` enforced
/// via [`Config::Asset`]
pub type ActualAsset<T> = <<T as Config>::Asset as Inspect<AuthorOf<T>>>::Balance;

/// Represents a singular election weight of numerous weights holded by the author.
///
/// Raw or intermediate election metric type
pub type ElectionWeight<T> =
    <<T as Config>::ElectionAdapter as ElectionManager<AuthorOf<T>>>::ElectionWeight;

/// Type representing the election weight of an author.
///
/// - Used to rank, score, or prioritize authors in elections.
/// - Typically derived from participation, stake, or other metrics.
/// - Serves as the primary metric for electing authors.
pub type ElectionVia<T> =
    <<T as Config>::ElectionAdapter as ElectionManager<AuthorOf<T>>>::ElectionWeightOf;

/// Parameters for configuring an election i.e, the input authors and their
/// corresponding election weight.
pub type ElectionParams<T> =
    <<T as Config>::ElectionAdapter as ElectionManager<AuthorOf<T>>>::Params;

/// Set of authors successfully elected by the author-role election module.
///
/// - Can be iterated over to assign points, rewards, or responsibilities.
/// - Typically used to determine which authors are active for a reward cycle
///   or session.
pub type ElectionElects<T> =
    <<T as Config>::ElectionAdapter as ElectionManager<AuthorOf<T>>>::Elected;

/// Represents a **collection of authors and their ephemeral points**
/// (e.g., block producer points) for a single reward cycle.
///
/// - Each entry is `(Author, Points)`.
/// - Serves as input to the [`Config::RewardModel`] plugin to compute per-author payouts.
/// - Points are temporary and cleared after the cycle.
pub type PayoutFor<T> = Vec<(AuthorOf<T>, <T as Config>::Points)>;

/// Represents a **collection of authors and their final rewards**
/// for a given reward cycle.
///
/// - Each entry is `(Author, Asset)`.
/// - Produced by the [`Config::RewardModel`] plugin after distributing total payout
///   based on ephemeral points.
/// - Used to execute the actual reward transfers or minting.
pub type PayeeList<T> = Vec<(AuthorOf<T>, AssetOf<T>)>;

/// Represents a **single penalty as a proportion** of a total value.
///
/// - Uses [`PerThing`](sp_runtime::PerThing) to allow fine-grained fractional penalties.
/// - Ideal for proportional deductions from author-risked assets for block-production.
/// - Ensures consistency and flexibility across different asset scales.
pub type PenaltyOf<T> = <<T as Config>::RoleAdapter as CompensateRoles<AuthorOf<T>>>::Ratio;

/// Collection of authors and their proportional penalties for a given cycle.
///
/// - Each entry represents `(Author, PenaltyOf)`.
/// - Input for [`Config::PenaltyModel`] plugin and output for transformed penalties.
/// - Supports multiple penalties applied in a single cycle.
pub type PenaltyFor<T> = Vec<(AuthorOf<T>, PenaltyOf<T>)>;

/// Represents the index of a block production session.
///
/// - Typically increments with each session in the runtime.
/// - Used to track author activity, points, rewards, or penalties per session.
pub type SessionIndex = sp_staking::SessionIndex;

/// Type alias for the **Validator ID** used by [`pallet_session`].
///
/// In the context of elections, candidate authors are mapped to this type
/// to interact with the session pallet (e.g., for validator set queries or election participation).
pub type SessionId<T> = <T as pallet_session::Config>::ValidatorId;

/// Type alias representing an **offending validator** in session history.
pub type Offender<T> = pallet_session::historical::IdentificationTuple<T>;

/// Type alias representing the **reporting account** that detected the offence.
pub type OffenceReporter<T> = <T as frame_system::Config>::AccountId;

/// The **runtime-storable identifier** of an affidavit key.
///
/// This represents the public identity of an affidavit key **as stored on-chain**.
/// While application crypto operates on raw public keys, the runtime persists
/// this identifier in the runtime's `AccountId` format for consistency and
/// interoperability with other pallets.
pub type AffidavitId<T> = <T as frame_system::pallet::Config>::AccountId;

/// Type representing **relative session duration**.
///
/// Used to define timing of affidavit and election phases
/// as a fraction of the current session.
///
/// Interpreted as a percentage ([`PerThing`](sp_runtime::PerThing)) of session length:
///
/// `block = session_start + (Duration * session_length)`
pub type Duration = Permill;

/// Type representing the **penalty ratio** applied to a author for
/// bad-behaviour.
///
/// Expressed as a [`Perbill`], aligning with
/// [`OnOffenceHandler`](sp_staking::offence::OnOffenceHandler)
/// which applies penalties directly using per-bill fractions.
pub type PenaltyRatio = Perbill;

// ===============================================================================
// `````````````````````````````````` ROUTINES ```````````````````````````````````
// ===============================================================================

/// Operational context for **affidavit key initialization**.
///
/// This type represents the contextual information required to initialize
/// or recover an affidavit key during the key lifecycle.
///
/// Affidavit keys are **ephemeral, operational keys** that are rotated
/// independently of long-term authority, stash, or consensus keys.
/// Initialization may execute sequentially multiple times as a looped
/// routine (e.g. via offchain workers), and therefore requires an explicit,
/// lightweight context object.
///
/// ## Notes
/// - This is **not a transaction payload**.
/// - This type is never submitted on-chain.
/// - It is used internally during:
///   - affidavit key generation
///   - key recovery
///   - keystore initialization or repair
///
/// ## Fork Awareness
/// - The `at` field captures the block number at which the initialization
///   process begins.
/// - When executed via offchain workers, this context must tolerate
///   forks, re-orgs, and speculative execution.
/// - Implementors are responsible for ensuring idempotency and re-entrancy safety.
///
/// ## Initialization Flow
///
/// The process repeatedly attempts to resolve a affidavit key-pair. It
/// self-heals by creating and inserting keys when missing, and retries
/// on missing or error states until a consistent state is reached.
///
/// ## Pseudocode
///
/// ```ignore
/// loop {
///     // Fetch the tagged active affidavit from offchain storage
///     match offchain_storage.fetch_active_affidavit() {
///         Ok(None) => {
///             // No active-tagged affidavit-key exists -> create a new key-pair locally
///             keystore.create_affidavit_key();
///
///             // Publish the key reference to offchain storage
///             offchain_storage.insert_affidavit_key();
///             continue;
///         }
///         Ok(Some(affidavit)) => {
///             // A active-tagged affidavit exists -> ensure the local key-pair is present
///             match keystore.get_affidavit_key() {
///                 Some(key) => break key,
///                 None => {
///                     // Storage provided public key, but its actual key-pair
///                     // is missing locally -> repair
///                     keystore.create_affidavit_key();
///                     continue;
///                 }
///             }
///         }
///         Ok(Some(_other_status)) => continue,
///         Err(_storage_error) => continue, // transient storage error -> retry
///     }
/// }
/// ```
///
/// ## Guarantee
///
/// This process yields an active affidavit key **only when** its tagged/referenced
/// via an offchain storage and its corresponding the key-pair exists in the
/// local keystore. Otherwise, it creates/repairs the key and keeps retrying
/// until the affidavit becomes tagged and the keystore is consistent.
///
/// ```ignore
/// loop {
///     if offchain_storage.has_affidavit_key()
///         && keystore.has_affidavit_key()
///     {
///         break;
///     }
///
///     keystore.create_or_repair_affidavit_key();
///     offchain_storage.ensure_affidavit_key_reference();
///     // retry until next key reaches consistency
/// }
/// ```
///
/// All behavior is supplied by trait implementations operating on this type.
///
/// ## FlowChart
///
/// [![](https://mermaid.ink/img/pako:eNqFUsFu2zAM_RWBpwxIMseNPUeHAVnaDsWwtWh72ryDYjO2MFsOZDldmuTfR1lxWjfAqoNEkXyP5JN2kFQpAodVUT0ludCGPV7GKlaM1oOh-68Y2pOzGyXNfLWSqdhI8w23gw8x_O5y62aZabHOWQynHEZJLUqKQj4LIysVg0u3K5UaE-tkj19evNdokpyKtiebU8IG2Qvl4MFUWmTI7pplIRNbwrXR4RcahUEicAb7gU-s39FgobdrUzHLhH3wjaqxHdkZb4Hv1f6KVheC36PREnuNv1P5HkVqkT3A6E5IzeYbIQuxLPAEQJV2ul-plFC0c6IwjVbnkhHRq4dqH5ONRp-d0p3byU3u_XdZ11Jl-6OSLnwU08KcNM59lOk_bNdVo9L9URkXdPbraDv8WfCskbfcV1pXmn1ktyZHbQczTb3vN9Iyt_2RRDCETMsUuNENDqFEXQp7hZ1NjoFoShKZk5kK_cf-1QNh1kL9rKqyg-mqyXLgK1HUdGvWKfV2KQX9_vLk1fRCqBc0nQHu-9OWBPgO_gKfzcZ-MJ14QeBPonASfRrCFnh0MY4u_Cj0oij0Z14QHYbw3Fb1xmEQTYNgQtHIm4bT2eEfZkQtCQ?type=png)](https://mermaid.live/edit#pako:eNqFUsFu2zAM_RWBpwxIMseNPUeHAVnaDsWwtWh72ryDYjO2MFsOZDldmuTfR1lxWjfAqoNEkXyP5JN2kFQpAodVUT0ludCGPV7GKlaM1oOh-68Y2pOzGyXNfLWSqdhI8w23gw8x_O5y62aZabHOWQynHEZJLUqKQj4LIysVg0u3K5UaE-tkj19evNdokpyKtiebU8IG2Qvl4MFUWmTI7pplIRNbwrXR4RcahUEicAb7gU-s39FgobdrUzHLhH3wjaqxHdkZb4Hv1f6KVheC36PREnuNv1P5HkVqkT3A6E5IzeYbIQuxLPAEQJV2ul-plFC0c6IwjVbnkhHRq4dqH5ONRp-d0p3byU3u_XdZ11Jl-6OSLnwU08KcNM59lOk_bNdVo9L9URkXdPbraDv8WfCskbfcV1pXmn1ktyZHbQczTb3vN9Iyt_2RRDCETMsUuNENDqFEXQp7hZ1NjoFoShKZk5kK_cf-1QNh1kL9rKqyg-mqyXLgK1HUdGvWKfV2KQX9_vLk1fRCqBc0nQHu-9OWBPgO_gKfzcZ-MJ14QeBPonASfRrCFnh0MY4u_Cj0oij0Z14QHYbw3Fb1xmEQTYNgQtHIm4bT2eEfZkQtCQ)
///
#[derive(Encode, Decode, Clone, MaxEncodedLen, TypeInfo, PartialEq, Eq)]
#[scale_info(skip_type_params(T))]
pub(crate) struct InitAffidavitKey<T: Config> {
    /// Block number at which affidavit key initialization begins.
    ///
    /// Used for logging, diagnostics, and fork-aware coordination.
    pub at: BlockNumberFor<T>,
}

/// Operational context for **election transaction execution**.
///
/// This type represents the contextual information required to attempt
/// authors election using an affidavit key during the routine lifecycle.
///
/// Election attempts are **optimistic, sequential routines** that may execute
/// repeatedly as part of a looped execution model (e.g. via offchain workers).
/// Although election typically follows the affidavit declaration phase,
/// explicit looping of sequential routines allows this phase to run earlier.
/// The routine remains safe, as successful election is gated by retrievable
/// storage state, election window constraints, and eligibility checks.
///
/// ## Notes
/// - This is **not a transaction payload**.
/// - This type is never submitted on-chain.
/// - It is used internally during:
///   - fetching the active-tagged affidavit key (storage-reference + keystore-pair)
///   - election payload signing using the affidavit key pair
///   - submission of the `elect` extrinsic
///   - retry-driven OCW election execution
///
/// ## Fork Awareness
/// - The `at` field captures the block number at which the election
///   attempt begins.
/// - When executed via offchain workers, this context must tolerate
///   forks, re-orgs, and speculative execution.
/// - Implementors are responsible for ensuring idempotency and
///   re-entrancy safety.
///
/// ## Dependency: [`InitAffidavitKey`]
///
/// This routine depends on the affidavit key initialization phase.
/// It assumes that a active-tagged (offchain storage referenced) affidavit key
/// and its pair in crypto-store is available. Whenever this invariant is violated
/// (e.g. missing key, failed signing, or inconsistent state), control is redirected
/// to [`InitAffidavitKey`] to repair and re-validate the key before retrying the
/// election attempt.
///
/// ## Election Flow
///
/// The routine fetches the active-tagged affidavit key-pair (referenced via
/// offchain storage and retrieved through local keystore), then attempts
/// author election. It short-circuits when outside the election window, when
/// ineligible, or when already elected (to avoid redundancy). Failures or
/// inconsistencies re-enter the affidavit initialization phase for re-validation
/// and repair.
///
/// ```ignore
/// loop {
///     // Ensure active-tagged affidavit key is available
///     InitAffidavitKey::ensure_active_affidavit_key();
///
///     // Fetch affidavit key pair (offchain storage reference + keystore-pair)
///     let key_pair = fetch_affidavit_key_pair();
///
///     if !within_election_window()
///         || !eligible_to_elect()
///         || already_elected()
///     {
///         break; // proceed to declaration phase (no election required)
///     }
///
///     let payload = Default::default();
///
///     match sign_payload_with(key_pair)
///         .and_then(|payload| submit_elect_authors_extrinsic(payload))
///     {
///         Ok(_) => break,     // elect-phase completed successfully
///         Err(_) => continue, // retry via initialization phase again
///     }
/// }
/// ```
///
/// ## Guarantees
///
/// This routine proceeds to the affidavit declaration phase under
/// exactly two conditions:
///
/// 1. **Initialization-only path**: a active-tagged affidavit key-pair
///    exists, but election constraints are not satisfied (outside window,
///    ineligible, or already elected).
///
/// 2. **Election-complete path**: a active-tagged affidavit key-pair
///    exists, election constraints are satisfied, and the `elect`
///    extrinsic is successfully submitted.
///
/// In all other cases, the routine retries by re-entering the affidavit
/// key initialization phase to re-validate offchain storage and
/// keystore consistency.
///
/// All behavior is supplied by trait implementations operating on this type.
///
/// ## FlowChart
///
/// [![](https://mermaid.ink/img/pako:eNp9VG1r2zAQ_itCMGhZ0sZ5cVMzOkKbQhmMsBTKNo-hWBdb1JGDLKdNk_z3naTYiZ0wf7Hke-65l-fOGxplHGhA52n2FiVMafL8EMpQEnymGu-_Q2rfAXlW63EKkRaZvLgM6Z8S9ukTabfbZDSfC85WQpNvsCZPUmjBUvHBDJ5MEpaDgTmXvJjFii0TC8MI__O1kcj-ebpPIHrdHHtMilkqIuv4Zaau7z6bY64zBWTChCKjFRMpm6XwNaS7I6YfsEQzBr9XwDSQa-K-GHdLxCQnY5kXSDRFOhYDIuagQEZwJims7m77PduWzOa-NzkoSN5omWunjMmo0Emm8jMNshBMsgmtJTBC0woQ9Qg6SupK2FIuqsTbphTgpNGlyxrfi5A8e8MuvwidCElK2feGeiPHqYgFtndjknRHojPnU0eOUuw0Xxv13MmBgNdhUxFLM3X4OkSesHWaMe6qWQnWGLfTGqbFbGFnyx0OTM-KyZzZs2Mbv2slZC6igIAB_WWuxQe-I-XGkpuRySSKUdhCHyBKmYKjgTSzfrofbgVsdGETqcS2-3UyLkdT9RPy7V7ksxOEZI-4vwdGh7WUTrLSzd2qUR2bwhrfbbBSyarsUtkTz5rFJerELV1LrQ_cpeuxxXAaxat_j1HfGKZFFEGObk7HhvURdxsXdFt1bm93mp9FNOwVv02LtmisBKeBVgW06ALUgpkr3RjHkOoEFqhtgEfO1GtIQ7lDnyWTv7JsUbqprIgTGsxZmuOtWHL8vzwIhhu9qL7iNnJQ91khNQ36fd-S0GBD32ngXfU7vW63e9Pvdwd-f9gbtOiaBm3P7115veHAH3Y6Pa93O7jZteiHDexdDX2v43WHnaHnebd939_9A4zO3PM?type=png)](https://mermaid.live/edit#pako:eNp9VG1r2zAQ_itCMGhZ0sZ5cVMzOkKbQhmMsBTKNo-hWBdb1JGDLKdNk_z3naTYiZ0wf7Hke-65l-fOGxplHGhA52n2FiVMafL8EMpQEnymGu-_Q2rfAXlW63EKkRaZvLgM6Z8S9ukTabfbZDSfC85WQpNvsCZPUmjBUvHBDJ5MEpaDgTmXvJjFii0TC8MI__O1kcj-ebpPIHrdHHtMilkqIuv4Zaau7z6bY64zBWTChCKjFRMpm6XwNaS7I6YfsEQzBr9XwDSQa-K-GHdLxCQnY5kXSDRFOhYDIuagQEZwJims7m77PduWzOa-NzkoSN5omWunjMmo0Emm8jMNshBMsgmtJTBC0woQ9Qg6SupK2FIuqsTbphTgpNGlyxrfi5A8e8MuvwidCElK2feGeiPHqYgFtndjknRHojPnU0eOUuw0Xxv13MmBgNdhUxFLM3X4OkSesHWaMe6qWQnWGLfTGqbFbGFnyx0OTM-KyZzZs2Mbv2slZC6igIAB_WWuxQe-I-XGkpuRySSKUdhCHyBKmYKjgTSzfrofbgVsdGETqcS2-3UyLkdT9RPy7V7ksxOEZI-4vwdGh7WUTrLSzd2qUR2bwhrfbbBSyarsUtkTz5rFJerELV1LrQ_cpeuxxXAaxat_j1HfGKZFFEGObk7HhvURdxsXdFt1bm93mp9FNOwVv02LtmisBKeBVgW06ALUgpkr3RjHkOoEFqhtgEfO1GtIQ7lDnyWTv7JsUbqprIgTGsxZmuOtWHL8vzwIhhu9qL7iNnJQ91khNQ36fd-S0GBD32ngXfU7vW63e9Pvdwd-f9gbtOiaBm3P7115veHAH3Y6Pa93O7jZteiHDexdDX2v43WHnaHnebd939_9A4zO3PM)
///
#[derive(Encode, Decode, Clone, MaxEncodedLen, TypeInfo, RuntimeDebug, PartialEq, Eq)]
#[scale_info(skip_type_params(T))]
pub(crate) struct TryElection<T: Config> {
    /// Public key authorized to sign election transactions.
    pub by: T::Public,

    /// Block number at which election execution is initiated.
    pub at: BlockNumberFor<T>,
}

/// Operational context for **affidavit declaration and key rotation**.
///
/// This type represents the contextual information required to declare
/// an affidavit using the active affidavit key and rotate it to the next key
/// during the routine lifecycle.
///
/// Declaration is a **sequential, retry-driven routine** that executes as part
/// of the looped offchain workflow. It is entered only after the initialization
/// phase guarantees that a active-tagged affidavit key-pair is available, and
/// after the election phase has either completed successfully or been safely
/// skipped due to unmet constraints.
///
/// ## Notes
/// - This is **not a transaction payload**.
/// - This type is never submitted on-chain.
/// - It is used internally during:
///   - next affidavit key resolution
///   - declaration payload composition (including next public key for rotation)
///   - payload signing using the active affidavit key-pair
///   - submission of the declare-affidavit extrinsic
///
/// ## Dependency: [`InitAffidavitKey`] and optimistic [`TryElection`]
///
/// This routine has a hard dependency on the initialization phase and a
/// soft (optimistic) dependency on the election phase.
///
/// It requires that:
/// - a *active-tagged* affidavit key exists in offchain storage and
///   the its key-pair  in local keystore (guaranteed by [`InitAffidavitKey`]), and
/// - election has either completed successfully or has been safely skipped
///   due to unmet constraints.
///
/// Election may execute before declaration as part of the global retry loop,
/// but its outcome does not directly gate declaration correctness. However,
/// failures during election invalidate system invariants and therefore
/// redirect control back to [`InitAffidavitKey`] for repair and re-validation
/// before declaration is retried.
///
/// ## Fork Awareness
/// - The `at` field captures the block number at which the declaration
///   process begins.
/// - When executed via offchain workers, this context must tolerate
///   forks, re-orgs, and speculative execution.
/// - Implementors must ensure idempotency and re-entrancy safety.
///
/// ## Declaration Flow
///
/// The routine fetches the active-tagged affidavit key-pair, resolves the
/// next affidavit key (mirroring the affidavit-key-initialization logic),
/// verifies the declaration window and eligibility, composes the declaration
/// payload along with the next affidavit public key, signs it with the active
/// affidavit key-pair, and submits the declaration transaction to rotate the
/// active affidavit key to next affidavit key.
///
/// Failures or inconsistencies re-enter the initialization phase to
/// re-validate offchain storage and keystore invariants.
///
/// ```ignore
/// loop {
///     // Ensure active-tagged affidavit key-pair is available
///     InitAffidavitKey::ensure_active_affidavit_key();
///
///     // Fetch affidavit key pair (offchain storage reference + keystore-pair)
///     let key_pair = fetch_affidavit_key_pair();

///     // Attempt election optimistically; failure requires repair + retry
///     if TryElection::attempt_election_if_applicable().is_err() {
///         continue; // re-enter initialization phase
///     }
///
///     // Resolve next affidavit key using the offchain-storage references +
///     // keystore pair consistency guarantees as InitAffidavitKey
///     if !offchain_storage.has_next_affidavit_key()
///         || !keystore.has_next_affidavit_key()
///     {
///         keystore.create_or_repair_next_affidavit_key();
///         offchain_storage.ensure_next_affidavit_key_reference();
///         continue; // retry until next key reaches consistency
///     }
///
///     let next_key_to_rotate = fetch_next_affidavit_key();
///
///     if !within_affidavit_window() || !eligible_to_declare() {
///         continue; // retry via initialization phase
///     }
///
///     let payload = compose_declare_affidavit_payload(next_key_to_rotate);
///
///     match sign_payload_with(key_pair)
///         .and_then(|payload| submit_declare_affidavit_extrinsic(payload)) {
///         Ok(_) => break,     // declaration + rotation completed
///         Err(_) => continue, // retry via initialization phase
///     }
/// }
/// ```
///
/// ## Guarantee
///
/// This routine declares the affidavit and rotates the active key
/// **only when**:
/// - a active-tagged affidavit key-pair is available,
/// - the next affidavit key is initiated and present in the local keystore,
/// - declaration window and eligibility constraints are satisfied,
/// - and the declaration transaction is successfully submitted.
///
/// In all other cases, the routine retries by re-entering the affidavit
/// key initialization phase to repair and re-validate offchain storage
/// and keystore consistency.
///
/// All behavior is supplied by trait implementations operating on this type.
///
/// ## FlowChart
///
/// [![](https://mermaid.ink/img/pako:eNqlVdtuGzcQ_RWCQAILsRLZli1bKBIYsgwYQV3DEhC0VZFQuyMt4RUpkFxbiuX3_kn_q1_S4WVv2k2QInrRkpw5Mzwzc_hMIxkDHdJFKp-ihClDplczMRMEfxOD6z9n1P0PyRVEKVNwuVjwmD1yc9CZ0b9y21evSLfbJcUh-QhbciO44SzlX5nhUlgDb6yz-VKxdeIMMMD3vFwMEn43owSih2f0iAx_hL1wv8zVu_dv7Kc2UgG5Y1yRy0fGUzZP4cOMvlSQ7mGNxxh8pIAZIO-I3yEBGlEcHhMxGQudId4EUdkS0HABCkQELbnhJd_vbuUuD2DX4cibgoj3OBungCHFklxmJpFKt_DkTDDXfdNaAuOcHG-FhI-k0EYxLowmE2RTLzjEH9y1Dj5xEcsn8u_f_yA6X3JkyC1upcELxpmImTCdGmfj6cZ2QzZfIeFfwEb5zHwiX8h4YxQXmkdFTs2b-g5y-eeF0-TAlu5eGt8jdwppU-6700LELWwsD6EVK_V_HTyhgKpx44uKntdgooS0tY-npVHjmOx1VKcO7HlE1j9xk3BRwfQn9ba7zLl2ZQq0G5nPljcuze11XcZF5nbne3nfZfOUR3a_nqf1851etvy3sEZqu8acLCQ0UW6EBicL_uOnMroHbBpwhck__0dWJdZIrtZSu6v5r5zQbgl0x7apZLEHuxFRmsWNYGWuZCFV0Ur1xCd8Kewc4F8d9JGzinq0NIsfnatyiJp9PFVMaBbVGrgySGMRo7cz4m5ekBGXZSnLLnTCNDTVeSyM2pZT5XS9JlA1awdCylgVYapo3e-gd0F6Kv6FBK2ZSXTIvXSa0VuZF6Iq9l0p0q3zQeJ2gcyGtw853YSD6SZgTrIoAq09cJ5BN8KOSAHbvR02eF_jK4Eav2tS4Wvk1enJi2ZkTcKtQsEti8W0Vrxdgz1gSRRomWYO5WDFlbJCz-vvYyqXPOp41wLLZfcr1xplc1cZ4yJiGMO9BJoY1xI1fVcbuzazsU0NX8PfTALKtojJdAsrhWC_JnFJUAlYjHMldNBKEogLq-LBrDyS1TNX7UI3w3mhna3etVPnH2ThG_kV9FZRciWxxNpZDzNjp976hGbb5UO9d9zop8r8_5ABKSPg0NNDulQ8pkOjMjikK1ArZpf02TrOKBZrhQM_xM-YqYcZnYkX9Fkz8YeUq9xNyWyZ0OGCpRpX2TrGPrriDJ_WVbGLj14MaoQVM3R4dHzWcyh0-Ew3uD7vvz09HfTwd3J60Ts5GhzSLR1e4O7g9GzQG5wcn_f7g8HLIf3q4vbeXvRxC83P-r3z_tHx4OU_ZolmWg?type=png)](https://mermaid.live/edit#pako:eNqlVdtuGzcQ_RWCQAILsRLZli1bKBIYsgwYQV3DEhC0VZFQuyMt4RUpkFxbiuX3_kn_q1_S4WVv2k2QInrRkpw5Mzwzc_hMIxkDHdJFKp-ihClDplczMRMEfxOD6z9n1P0PyRVEKVNwuVjwmD1yc9CZ0b9y21evSLfbJcUh-QhbciO44SzlX5nhUlgDb6yz-VKxdeIMMMD3vFwMEn43owSih2f0iAx_hL1wv8zVu_dv7Kc2UgG5Y1yRy0fGUzZP4cOMvlSQ7mGNxxh8pIAZIO-I3yEBGlEcHhMxGQudId4EUdkS0HABCkQELbnhJd_vbuUuD2DX4cibgoj3OBungCHFklxmJpFKt_DkTDDXfdNaAuOcHG-FhI-k0EYxLowmE2RTLzjEH9y1Dj5xEcsn8u_f_yA6X3JkyC1upcELxpmImTCdGmfj6cZ2QzZfIeFfwEb5zHwiX8h4YxQXmkdFTs2b-g5y-eeF0-TAlu5eGt8jdwppU-6700LELWwsD6EVK_V_HTyhgKpx44uKntdgooS0tY-npVHjmOx1VKcO7HlE1j9xk3BRwfQn9ba7zLl2ZQq0G5nPljcuze11XcZF5nbne3nfZfOUR3a_nqf1851etvy3sEZqu8acLCQ0UW6EBicL_uOnMroHbBpwhck__0dWJdZIrtZSu6v5r5zQbgl0x7apZLEHuxFRmsWNYGWuZCFV0Ur1xCd8Kewc4F8d9JGzinq0NIsfnatyiJp9PFVMaBbVGrgySGMRo7cz4m5ekBGXZSnLLnTCNDTVeSyM2pZT5XS9JlA1awdCylgVYapo3e-gd0F6Kv6FBK2ZSXTIvXSa0VuZF6Iq9l0p0q3zQeJ2gcyGtw853YSD6SZgTrIoAq09cJ5BN8KOSAHbvR02eF_jK4Eav2tS4Wvk1enJi2ZkTcKtQsEti8W0Vrxdgz1gSRRomWYO5WDFlbJCz-vvYyqXPOp41wLLZfcr1xplc1cZ4yJiGMO9BJoY1xI1fVcbuzazsU0NX8PfTALKtojJdAsrhWC_JnFJUAlYjHMldNBKEogLq-LBrDyS1TNX7UI3w3mhna3etVPnH2ThG_kV9FZRciWxxNpZDzNjp976hGbb5UO9d9zop8r8_5ABKSPg0NNDulQ8pkOjMjikK1ArZpf02TrOKBZrhQM_xM-YqYcZnYkX9Fkz8YeUq9xNyWyZ0OGCpRpX2TrGPrriDJ_WVbGLj14MaoQVM3R4dHzWcyh0-Ew3uD7vvz09HfTwd3J60Ts5GhzSLR1e4O7g9GzQG5wcn_f7g8HLIf3q4vbeXvRxC83P-r3z_tHx4OU_ZolmWg)
///
#[derive(Encode, Decode, Clone, MaxEncodedLen, RuntimeDebug, TypeInfo, PartialEq, Eq)]
#[scale_info(skip_type_params(T))]
pub(crate) struct DeclareAffidavit<T: Config> {
    /// Raw application public key identifying the affidavit key-pair
    /// in the local keystore.
    pub by: T::Public,

    /// Block number at which the affidavit declaration process started.
    ///
    /// Used exclusively for logging and diagnostic purposes.
    pub at: BlockNumberFor<T>,
}

/// Operational context for **affidavit key rotation**.
///
/// This type represents the contextual information required to observe
/// and commence rotation of the affidavit key, promoting the previously
/// prepared *next* key to become the active affidavit key once the
/// declaration effects are finalized.
///
/// Rotation is a **sequential, retry-driven routine** that executes as part
/// of the looped offchain workflow. It is entered only after the declaration
/// phase has submitted the declare-affidavit transaction successfully.
///
/// ## Notes
/// - This is **not a transaction payload**.
/// - This type is never submitted on-chain.
/// - It is used internally during:
///   - observation of finalized declaration effects
///   - validation of next-key availability
///   - promotion of the next affidavit key to active
///   - reset of inconsistent key state when rotation cannot proceed
///
/// ## Dependency: [`InitAffidavitKey`], [`TryElection`], and [`DeclareAffidavit`]
///
/// This routine depends on the prior initialization, election, and declaration
/// phases. It assumes that:
/// - a *active-tagged* affidavit key-pair exists (guaranteed by [`InitAffidavitKey`]),
/// - election has either completed or been safely skipped (handled by [`TryElection`]),
/// - a declare-affidavit transaction has been submitted (by [`DeclareAffidavit`]).
/// - and a *next-tagged* affidavit key-pair exists,
///
/// Whenever these invariants are violated (e.g. missing affidavit keys,
/// eligibility failure, or window violations), control is redirected to
/// [`InitAffidavitKey`] to repair and re-validate storage and keystore
/// consistency before retrying rotation observation.
///
/// ## Fork Awareness
/// - The `at` field captures the block number at which rotation observation
///   begins.
/// - When executed via offchain workers, this context must tolerate forks,
///   re-orgs, and speculative execution.
/// - Implementors must ensure idempotency and re-entrancy safety.
///
/// ## Rotation Flow
///
/// The routine verifies that the next affidavit key is referenced in offchain
/// storage and locally available as pair in keystore, then determines whether
/// rotation is currently eligible. If so, the next key is promoted to active
/// and the previous state is cleaned up. If rotation cannot occur within the
/// allowed session window, both active and next keys are reset to recover from
/// inconsistent or stale state.
///
/// Failures or inconsistencies at any stage re-enter the initialization
/// phase to re-validate offchain storage and keystore invariants.
///
/// ```ignore
/// loop {
///     // --- Initialization Phase ---
///     if !offchain_storage.has_active_affidavit_key()
///         || !keystore.has_active_affidavit_key()
///     {
///         keystore.create_or_repair_active_key();
///         offchain_storage.ensure_active_key_reference();
///         continue;
///     }
///
///     // --- Election Phase (optimistic) ---
///     if election_constraints_satisfied() {
///         if submit_elect_authors_extrinsic().is_err() {
///             continue; // repair + retry via initialization
///         }
///     }
///
///     // --- Declaration Phase ---
///     if !offchain_storage.has_next_affidavit_key()
///         || !keystore.has_next_affidavit_key()
///     {
///         match keystore.create_or_repair_next_key() {
///             Ok(_) => {
///                 offchain_storage.ensure_next_key_reference();
///                 continue;
///             }
///             Err(_) => continue, // fallback to initialization repair loop
///         }
///     }
///
///     if !affidavit_constraints_satisfied() {
///         continue; // retry via initialization phase
///     }
///
///     if submit_declare_affidavit_extrinsic().is_err() {
///         continue; // declaration failed -> repair + retry
///     }
///
///     // --- Rotation Observation Phase ---
///     if !offchain_storage.has_next_affidavit_key()
///         || !keystore.has_next_affidavit_key()
///     {
///         continue;
///     }
///
///     if eligible_to_rotate() {
///         promote_next_key_to_active();
///         remove_next_key();
///         break; // rotation completed
///     }
///
///     if within_current_session_window() {
///         continue; // wait until rotation becomes eligible
///                   // indicates declaration effects are not finalized
///     }
///
///     // Window expired -> reset inconsistent state
///     remove_active_and_next_affidavit_keys();
///     break;
/// }
/// ```
///
/// ## Guarantee
///
/// The rotation phase has exactly three exit outcomes:
///
/// 1. **Rotation Success**:
///    - if the next affidavit key is finalized and present in the keystore, and
///    - rotation eligibility constraints are satisfied,
///    then the next key is promoted to become the active affidavit key.
///
/// 2. **Deferred Rotation (Same Session)**:
///    - if the next affidavit key is finalized but not yet eligible to rotate, and
///    - the current session window in which the affidavit was declared is still active,
///    the routine keeps retrying until eligibility is satisfied and rotation succeeds.
///
/// 3. **Reset & Repair (Session Elapsed)**:
///    - if rotation is still ineligible and the declaring session window has elapsed,
///    both active and next affidavit keys are removed, signaling that the declaration
///    phase effectively failed to converge. The global loop then re-enters the
///    initialization phase to repair and re-establish a consistent state.
///
/// ```ignore
/// loop {
///
///     if eligible_to_rotate() {
///         promote_next_key_to_active();
///         remove_next_affidavit_key();
///         break;
///     }
///     if within_declaring_session_window() {
///         continue;
///         // keep retrying until eligibility becomes true
///     } else {
///         remove_active_and_next_affidavit_keys();
///         break;
///         // signal reset; initialization phase will repair state
///     }
/// }
/// ```
///
/// All behavior is supplied by trait implementations operating on this type.
///
/// ## Timelines & Window Semantics
///
/// The global retry loop operates over session-scoped time windows that
/// determine when election, declaration, and rotation are allowed.
///
/// Timeline within a single session:
///
/// ```text
/// |--------------------- Session N ---------------------|
///        |--------- Affidavit Window ---------|
///                 |----- Election Window -----|
///
/// Session Start
///     |- Affidavit window opens
///          |- Election window opens (nested inside affidavit window)
///
/// Election window closes
///     |- Election no longer allowed
///
/// Affidavit window closes
///     |- Declaration and rotation eligibility must already be satisfied
///
/// Session ends -> Session N+1 begins
///     |- Global loop continues with fresh window constraints
/// ```
///
/// Properties:
/// - The **election window** is strictly nested within the **affidavit window**.
/// - When the affidavit window ends, the election window has already ended.
/// - Rotation eligibility is evaluated relative to the session in which the
///   affidavit was declared.
/// - If rotation has not converged before the session window elapses, the
///   routine resets keys and relies on the next session's loop iteration
///   to re-initialize and re-attempt the full lifecycle.
///
/// ## FlowChart
///
/// [![](https://mermaid.ink/img/pako:eNqtVdtu2zgQ_RWCQBctNk7tyk4coWgRxCoQLOAt7ADFblW0tDi2iEqkQVGu0zivi33en9j_6pfskLpZsl2gxfolkXjmzMzhmdEDjRQH6tNlor5EMdOG3E1CGUqCv7nB5_chdX99MlOGGbheLgVnG2F-g_unz0L6oUI_eUJ6vR6pzwkCyK0URrBEfGVGKGkBBTjLFyvN1rEDYIrvRbkcpPzd3sQQfX4I6RshLQI4uY6M2ECT-OVCP3_1qyXKjNLgGK83TCRskcDrkD7usc1gzYTGAm40YHPkOSneVKQY6-iY5CSQWY50cyRlK0DgEjTICI7Uh42-2k3Vrkpgn8ujAgqSd3QLEsCUckWucxMrnR3RykGw1i60VUBQCVSgUPQbJTOjmZAmI3NUNFsK4K9dW0_fCcnVF_Lt73-RXawEKuQepspggzyXnEnzrKVZcLe1nsgXKd7WJ7BZPrKikE8k2BotZCaiuqbDTicQJUy7-utbP2x2Clvb6xFwq93poR9s5E-5YXrSDY7yh70wqWpr-vzBu2grP2kpz50y8JFV5F31uyLVppzumXI3z6MIsmxXgo4G_QF4Pume77O8QUFRkF1t8q4Kezx326OHblw6ORDrzg7ojzvL3q3bUtb2vy8y0JtTe6dYZijnfkzr9mb_o7Nm1YW6uSznzKiyig628AEi3wkTC5zfXKO3DJnjNdlmivNOUN3PW61Shd6tLEu-_fVPtc1-QaOmatMcthueQQZ25kpQHdRu2zXcMdjs-9c461iglqPFUQvT4IquTmFsqlKtFlE5SDXNQTl7AMfhGj8wVSC5XbaS-42p3sYsA5zidJ2AgcOvXyCNvm_s5r6creXfQhdsd5rhzFr6vaV_25Es6MbX633NTJyVFTdBIZ2qYq20P6Y9JZN7F4O7pT30QTdlNahBOYV297hlURBXFfSiUo4TtKdmuG6l2PGFwFmRgSTAeGZHRFfSq2aei8hqOdQbbNYlrq9N5QaLhFKm0lbuXgJ73437q3f0jK604NQ3OoczmoJOmX2kDxYdUhNDitfv47-c6c8hDeUjxqyZ_FOptArTKl_F1F-yJMOnfM0x60Qw3EBp_RZHm4O-Ubk01L8YDl44Fuo_0C31X1xenHtX3mh8MR57nnc5GJ3Re4QNzi9HV8Nh3xteja8G_dHjGf3q8vbPx0MPoaP-xaB_6Xl97_E_dswmYg?type=png)](https://mermaid.live/edit#pako:eNqtVdtu2zgQ_RWCQBctNk7tyk4coWgRxCoQLOAt7ADFblW0tDi2iEqkQVGu0zivi33en9j_6pfskLpZsl2gxfolkXjmzMzhmdEDjRQH6tNlor5EMdOG3E1CGUqCv7nB5_chdX99MlOGGbheLgVnG2F-g_unz0L6oUI_eUJ6vR6pzwkCyK0URrBEfGVGKGkBBTjLFyvN1rEDYIrvRbkcpPzd3sQQfX4I6RshLQI4uY6M2ECT-OVCP3_1qyXKjNLgGK83TCRskcDrkD7usc1gzYTGAm40YHPkOSneVKQY6-iY5CSQWY50cyRlK0DgEjTICI7Uh42-2k3Vrkpgn8ujAgqSd3QLEsCUckWucxMrnR3RykGw1i60VUBQCVSgUPQbJTOjmZAmI3NUNFsK4K9dW0_fCcnVF_Lt73-RXawEKuQepspggzyXnEnzrKVZcLe1nsgXKd7WJ7BZPrKikE8k2BotZCaiuqbDTicQJUy7-utbP2x2Clvb6xFwq93poR9s5E-5YXrSDY7yh70wqWpr-vzBu2grP2kpz50y8JFV5F31uyLVppzumXI3z6MIsmxXgo4G_QF4Pume77O8QUFRkF1t8q4Kezx326OHblw6ORDrzg7ojzvL3q3bUtb2vy8y0JtTe6dYZijnfkzr9mb_o7Nm1YW6uSznzKiyig628AEi3wkTC5zfXKO3DJnjNdlmivNOUN3PW61Shd6tLEu-_fVPtc1-QaOmatMcthueQQZ25kpQHdRu2zXcMdjs-9c461iglqPFUQvT4IquTmFsqlKtFlE5SDXNQTl7AMfhGj8wVSC5XbaS-42p3sYsA5zidJ2AgcOvXyCNvm_s5r6creXfQhdsd5rhzFr6vaV_25Es6MbX633NTJyVFTdBIZ2qYq20P6Y9JZN7F4O7pT30QTdlNahBOYV297hlURBXFfSiUo4TtKdmuG6l2PGFwFmRgSTAeGZHRFfSq2aei8hqOdQbbNYlrq9N5QaLhFKm0lbuXgJ73437q3f0jK604NQ3OoczmoJOmX2kDxYdUhNDitfv47-c6c8hDeUjxqyZ_FOptArTKl_F1F-yJMOnfM0x60Qw3EBp_RZHm4O-Ubk01L8YDl44Fuo_0C31X1xenHtX3mh8MR57nnc5GJ3Re4QNzi9HV8Nh3xteja8G_dHjGf3q8vbPx0MPoaP-xaB_6Xl97_E_dswmYg)
///
#[derive(Encode, Decode, Clone, MaxEncodedLen, RuntimeDebug, TypeInfo, PartialEq, Eq)]
#[scale_info(skip_type_params(T))]
pub(crate) struct RotateAffidavitKey<T: Config> {
    /// Public key that is intended to become the new active affidavit key.
    pub by: T::Public,

    /// Block number at which key rotation is initiated.
    pub at: BlockNumberFor<T>,
}

// ===============================================================================
// `````````````````````````````` UNSIGNED PAYLOADS ``````````````````````````````
// ===============================================================================

/// Runtime-specialized payload type for affidavit declaration.
///
/// This is a concrete alias of [`AffidavitPayload`] using the runtime's
/// configured signing public key.
///
/// ## Usage
/// - Used by the pallet's unsigned `declare` extrinsic.
/// - Signed offchain using the active affidavit key.
/// - Verified on-chain via `ValidateUnsigned`.
///
/// ## Design Notes
/// - Separates **signing identity** (raw public key) from
///   **runtime identity** (account-based affidavit ID).
/// - Ensures payloads are always consistent with the runtime's
///   configured application crypto.
pub type AffidavitPayloadOf<T> = AffidavitPayload<<T as SigningTypes>::Public, AffidavitId<T>>;

/// Runtime-specialized payload type for affidavit key validation.
///
/// This is a concrete alias of [`ValidatePayload`] using the runtime's
/// configured signing public key.
///
/// ## Usage
/// - Used by the pallet's unsigned `validate` extrinsic.
/// - Proves possession of the active affidavit key prior to
///   affidavit declaration.
///
/// ## Design Notes
/// - Contains no mutable or session-specific data.
/// - Exists solely to authenticate the active affidavit key.
pub type ValidatePayloadOf<T> = ValidatePayload<<T as SigningTypes>::Public>;

/// Runtime-specialized payload type for author election execution.
///
/// This is a concrete alias of [`ElectionPayload`] using the runtime's
/// configured signing public key.
///
/// ## Usage
/// - Used by the pallet's unsigned `elect` extrinsic.
/// - Signed using the **currently active affidavit key**, which was
///   rotated during the latest affidavit declaration.
///
/// ## Design Notes
/// - Ensures only authors who successfully completed affidavit
///   declaration and key rotation can execute elections.
/// - Binds election authorization strictly to the runtime's
///   configured signing scheme.
pub type ElectionPayloadOf<T> = ElectionPayload<<T as SigningTypes>::Public>;

// ===============================================================================
// ```````````````````````````` GENESIS CONFIG UPDATE ````````````````````````````
// ===============================================================================

/// Enumerates configurable runtime-stored parameters that may be forcibly overridden
/// at runtime through privileged (root/governance) operations.
#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    Clone,
    RuntimeDebugNoBound,
    PartialEq,
    Eq,
    MaxEncodedLen,
    TypeInfo,
)]
#[scale_info(skip_type_params(T))]
pub enum ForceGenesisConfig<T: Config> {
    AllowAffidavits(bool),
    /// Updates the start of the affidavit submission window.
    AffidavitBeginsAt(Duration),
    /// Updates the end of the affidavit submission window.
    AffidavitEndsAt(Duration),
    /// Updates the point within the session when election execution begins.
    ElectionBeginsAt(Duration),
    /// Updates the points awarded for executing the election routine.
    ElectionRunnerPointsUpgrade(Option<T::Points>),
    /// Updates the transaction priority for validation-related extrinsics.
    ValidateTxPriority(TransactionPriority),
    /// Updates the transaction priority for election execution extrinsics.
    ElectionTxPriority(TransactionPriority),
    /// Updates the transaction priority for affidavit submission extrinsics.
    AffidavitTxPriority(TransactionPriority),
    /// Updates the time-based delay (in milliseconds) before operations are considered final.
    FinalityAfter(Moment<T>),
    /// Updates the block-based confirmation threshold for finality.
    FinalityTicks(BlockNumberFor<T>),
}

// ===============================================================================
// ``````````````````````````````` SESSION WINDOWS ```````````````````````````````
// ===============================================================================

/// Represents the **affidavit submission window** for the current session.
///
/// Defines the inclusive block range during which authors are permitted
/// to submit affidavits for the **next upcoming session**.
///
/// ### Semantics
///
/// - `start`: First block at which affidavit submission is allowed.
/// - `end`: Last block (inclusive) after which submissions are rejected.
#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    Clone,
    RuntimeDebugNoBound,
    PartialEq,
    Eq,
    MaxEncodedLen,
    TypeInfo,
)]
/// Affidavit Window for the current session
pub struct AffidavitWindow<T: Config> {
    /// Block number at which affidavit submission begins.
    pub start: BlockNumberFor<T>,
    /// Block number at which affidavit submission ends.
    pub end: BlockNumberFor<T>,
}

/// Represents the **election execution window** for the current session.
///
/// Defines the block range during which election logic is expected
/// to be executed for determining the **next session's validator set**.
///
/// ### Semantics
///
/// - `start`: First block at which election execution may begin.
/// - `end`: Final block for election execution.
#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    Clone,
    RuntimeDebugNoBound,
    PartialEq,
    Eq,
    MaxEncodedLen,
    TypeInfo,
)]
pub struct ElectionWindow<T: Config> {
    /// Block number at which election execution begins.
    pub start: BlockNumberFor<T>,
    /// Block number at which election execution ends.
    pub end: BlockNumberFor<T>,
}
