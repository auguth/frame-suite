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
// ````````````````````````````````` OCW ROUTINES ````````````````````````````````
// ===============================================================================

//! Offchain routines orchestrating the affidavit lifecycle and election execution.
//!
//! This module implements a coordinated set of Offchain Worker (OCW) routines
//! that drive the lifecycle of authors from affidavit key initialization to
//! election participation and key rotation.
//!
//! ## Lifecycle Pipeline
//!
//! The system operates as a continuous OCW-driven pipeline:
//!
//! ```text
//! InitAffidavitKey -> TryElection -> DeclareAffidavit -> RotateAffidavitKey
//! ```
//!
//! Each stage is independently executable and relies on repeated OCW execution
//! across blocks to eventually converge to a consistent state.
//!
//! ## Responsibilities
//!
//! ### 1. Affidavit Key Initialization (`InitAffidavitKey`)
//! - Generates an ephemeral affidavit key pair using application crypto.
//! - Persists the public identifier in [`Finalized`] offchain storage.
//! - Ensures exactly one active affidavit key exists per node.
//!
//! ### 2. Election Execution (`TryElection`)
//! - Opportunistically attempts to run the election for the upcoming session.
//! - Uses the currently active affidavit key for authorization.
//! - Ensures at-most-once execution per author per session.
//! - Designed to be non-blocking and retry-safe.
//!
//! ### 3. Affidavit Declaration (`DeclareAffidavit`)
//! - Submits a signed affidavit to signal participation in the next
//!   session's election.
//! - Prepares and finalizes the next affidavit key for rotation.
//! - Ensures eligibility and timing constraints via runtime checks.
//!
//! ### 4. Key Rotation (`RotateAffidavitKey`)
//! - Finalizes transition from next -> active affidavit key.
//! - Confirms successful affidavit submission via runtime state.
//! - Performs cleanup and ensures lifecycle continuity.
//!
//! ## Storage & Finality
//!
//! Uses layered offchain storage:
//! - [`ForkAware`]: fork-safe speculative state  
//! - [`Persistent`]: durable observation ledger  
//! - [`Finalized`]: stable values via [`Confidence`]
//!
//! Finality is governed by [`FinalizedPolicy`] using:
//! - time delay ([`FinalityAfter`])
//! - observation count ([`FinalityTicks`])
//!
//! ## Execution Model
//!
//! - **Idempotent**: All routines can run repeatedly without side effects.
//! - **Non-blocking**: Routines exit early when prerequisites are unmet.
//! - **Opportunistic**: Actions may be attempted before full readiness.
//! - **Eventually consistent**: Correct state is reached through repetition.
//!
//! ## Security Model
//!
//! - Uses **ephemeral affidavit keys** instead of long-term authority keys.
//! - Enforces **key rotation per lifecycle**.
//! - Limits signing scope to specific operations (affidavit/election).
//! - Reduces attack surface and key exposure risk.
//!
//! ## Failure Handling
//!
//! - Storage inconsistencies trigger **hard stops** to prevent unsafe execution.
//! - Failed extrinsics are logged and retried in future OCW runs.
//! - Missing runtime reflection (e.g. failed affidavit) triggers **state reset**.
//!
//! This ensures the system never remains in a partially inconsistent state.

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- Local crate imports ---
use crate::{
    crypto::*, types::*, AffidavitKeys, Config, CurrentSession, ElectsPreparedBy, Error,
    FinalityAfter, FinalityTicks, Internals, Pallet, SessionStartAt,
};

// --- FRAME Suite ---
use frame_suite::{ForksHandler, blockchain::*, routines::*};

// --- FRAME Support ---
use frame_support::traits::EstimateNextSessionRotation;

// --- FRAME System ---
use frame_system::{
    offchain::{AppCrypto, SendSignedTransaction, SignedPayload, Signer},
    pallet_prelude::BlockNumberFor,
};

// --- Substrate primitives ---
use sp_runtime::{
    DispatchError, RuntimeAppPublic, traits::{IdentifyAccount, One, Saturating}
};

//--- Scale-info ---
use scale_info::prelude::{format, string::String};

// ===============================================================================
// ```````````````````````````````` LOG CONSTANTS ````````````````````````````````
// ===============================================================================

/// Log target (classifier) for affidavit logging.
///
/// All pallet-specific offchain logs (affidavits) should
/// use this target to allow fine-grained filtering at the node level.
pub const LOG_TARGET_AFDT: Option<&'static str> = Some("AFFIDAVIT");

/// Log target (classifier) for elections logging.
///
/// All pallet-specific offchain logs (elections) should
/// use this target to allow fine-grained filtering at the node level.
pub const LOG_TARGET_ELEC: Option<&'static str> = Some("ELECTION");

// ===============================================================================
// ```````````````````````` INITIATE AFFIDAVIT KEY (OCW) `````````````````````````
// ===============================================================================

// --- Keys ---

/// Offchain key identifier for the **active affidavit key**.
///
/// Used as the base identifier for storing and retrieving the
/// currently active affidavit key from offchain storage.
///
/// The value stored under this key must survive fork re-orgs and
/// confidence evaluation before it is considered safe for usage.
pub const ACTIVE_AFDT_KEY: &'static [u8] = b"ACTIVE_AFDT_KEY";

// ===============================================================================
// ```````````````````````````````` LOG FORMATTER ````````````````````````````````
// ===============================================================================

/// Emoji indicator mapped to each [`LogLevel`] for visual log scanning.
const EMOJI_DEBUG: &str = "🐛";
const EMOJI_ERROR: &str = "🚨";
const EMOJI_INFO:  &str = "📣";
const EMOJI_WARN:  &str = "⚠️";

/// Returns the emoji indicator associated with a given [`LogLevel`].
///
/// Used internally by [`std_fmt`] to embed a visual severity cue
/// into the formatted log line.
#[inline(always)]
fn level_emoji(level: &LogLevel) -> &'static str {
    match level {
        LogLevel::Debug => EMOJI_DEBUG,
        LogLevel::Error => EMOJI_ERROR,
        LogLevel::Info  => EMOJI_INFO,
        LogLevel::Warn  => EMOJI_WARN,
    }
}

/// Standard log formatter for OCW routines.
///
/// Produces a consistently structured, human-readable log line that
/// embeds block context, severity, routing target, and message body.
///
/// ### Output format
///
/// ```text
/// 🧱 [<block>] <emoji> [<LEVEL>] 🎯 [<target>] 🧾 <message>
/// ```
///
/// ### Example
///
/// ```text
/// 🧱 [312] 📣 [Info] 🎯 [AFFIDAVIT] 🧾 Module(9): InitAffidavitKeyRoutineSuccess
/// ```
pub fn std_fmt<T: Config>(
    timestamp: BlockNumberFor<T>,
    level: &LogLevel,
    target: &str,
    message: &str,
) -> String {
    format!(
        "🧱 [{:?}] {} [{:?}] 🎯 [{}] 🧾 {}",
        timestamp,
        level_emoji(level),
        level,
        target,
        message
    )
}

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// ``````````````````````````````` ERROR PROVIDERS ```````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

/// Error mapping for **fork-aware speculative storage** access
/// during affidavit key initialization.
///
/// This implementation provides pallet-specific error variants
/// expected to be convertible into [`DispatchError`](sp_runtime::DispatchError).
///
/// ## Scope
/// - Applies to speculative, fork-aware storage keyed by a hash.
/// - Focuses on failures related to the speculative hash of the
///   finalized affidavit key value.
impl<T: Config> OffchainStorageError<ForkAware<T, ValueHash, InitAffidavitKey<T>, Pallet<T>>>
    for InitAffidavitKey<T>
{
    type Error = Error<T>;

    /// Speculative hash decoding failed.
    fn decode_failed() -> Self::Error {
        Error::<T>::ActiveAfdtKeySpeculativeHashDecodeFail
    }

    /// Concurrent mutation detected while accessing fork-aware storage.
    fn concurrent_mutation() -> Self::Error {
        Error::<T>::ActiveAfdtKeySpeculativeHashConcurrentMutation
    }
}

/// Error mapping for **persistent finalized offchain storage**
/// during affidavit key initialization.
///
/// This implementation handles failures when interacting with the
/// persistent ledger that stores finalized affidavit key values,
/// wrapped in [`Confidence`] to reflect finality guarantees.
///
/// ## Scope
/// - Applies to persistent, non-speculative storage.
/// - Covers decoding and concurrent mutation failures.
impl<T: Config> OffchainStorageError<Persistent<T, Ledger<T, AffidavitId<T>>, InitAffidavitKey<T>>>
    for InitAffidavitKey<T>
{
    type Error = Error<T>;
    /// Finalized value decoding failed.
    fn decode_failed() -> Self::Error {
        Error::<T>::ActiveAfdtKeyFinalizedValueDecodeFail
    }
    /// Concurrent mutation detected while accessing persistent storage.
    fn concurrent_mutation() -> Self::Error {
        Error::<T>::ActiveAfdtKeyFinalizedValueConcurrentMutation
    }
}

/// Invariant enforcement for **finalized offchain storage**.
///
/// This implementation defines errors for high-level coordination
/// failures between speculative and persistent storage layers.
///
/// Cleanups will be implicitly handled by the storage itself.
impl<T: Config> FinalizedOffchainStorageError<T, AffidavitId<T>> for InitAffidavitKey<T> {
    type Error = Error<T>;

    /// A speculative hash **must not exist** without a
    /// corresponding persistent value.
    fn hanging_hash() -> Self::Error {
        Error::<T>::ActiveAfdtKeySpeculativeHangingHash
    }

    /// A persistent value **must not exist** without holding its corresponding
    /// speculative hash's value.
    fn hanging_value() -> Self::Error {
        Error::<T>::ActiveAfdtKeyFinalizedHangingValue
    }
}

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// `````````````````````````````` FINALIZED POLICY ```````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

/// This policy defines when a finalized offchain storage value
/// is considered **safe for irreversible side effects**, such as:
/// - key promotion,
/// - state transitions,
/// - or cleanup of speculative storage.
///
/// The policy is consumed by the [`Finalized`] storage abstraction
/// in conjunction with [`Confidence`] to determine optimal-finality.
impl<T: Config> FinalizedPolicy<T> for InitAffidavitKey<T> {
    /// Returns the wall-clock time after which a value is considered final.
    fn finality_after() -> <T as pallet_timestamp::Config>::Moment {
        FinalityAfter::<T>::get()
    }

    /// Returns the number of block confirmations required to reach finality after
    /// the wall-clock time reached.
    fn finality_ticks() -> BlockNumberFor<T> {
        FinalityTicks::<T>::get()
    }
}

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// `````````````````````````````````` ROUTINES ```````````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

/// An Offchain Worker (OCW) routine responsible for initializing the
/// **active affidavit key** for the local node.
///
/// This routine bootstraps an affidavit application key **per node**,
/// regardless of whether the node is acting as an author i.e.,
/// validator node.
///
/// The affidavit key is a **rotated operational key** used exclusively
/// for affidavit-related signing. Only a single affidavit key is expected
/// to be active at any given time, and the runtime relies on this invariant
/// being upheld.
impl<T: Config> Routines<BlockNumberFor<T>> for InitAffidavitKey<T> {
    /// Determines whether the affidavit key initialization routine
    /// should run.
    ///
    /// Initialization **must not run** if:
    /// - an active affidavit key is already stored in the offchain storage, and
    /// - the corresponding key pair exists in the node's affidavit keystore (app crypto).
    ///
    /// Any storage inconsistency (e.g. corrupted or undecodable data)
    /// is treated as a **hard stop** and causes the routine to refuse
    /// execution, since proceeding could violate runtime expectations.
    fn can_run(&self) -> Result<(), Self::Logger> {
        // Optimistically retrieve the currently active affidavit key
        let result =
            Finalized::<T, AffidavitId<T>, Self, Pallet<T>>::get(ACTIVE_AFDT_KEY, LOG_TARGET_AFDT, None);

        // Only allow confident and safe storage values which are finalized as per our policy.
        let afdt_key = match result {
            Ok(None) => return Ok(()),
            Ok(Some(Confidence::Safe(key))) => key,
            Ok(Some(_)) => {
                return Err(<Self as Logging<BlockNumberFor<T>>>::debug(
                    &Error::<T>::ActiveAfdtKeyNotYetFinalized.into(),
                    self.at,
                    LOG_TARGET_AFDT,
                    Some(std_fmt::<T>),
                ))
            }
            Err(_) => {
                return Err(<Self as Logging<BlockNumberFor<T>>>::warn(
                    &Error::<T>::OCWStorageDecisionHalt.into(),
                    self.at,
                    LOG_TARGET_AFDT,
                    Some(std_fmt::<T>),
                ));
            }
        };

        // Retrieve all affidavit keys currently available in the node keystore
        let all_keys =
            <<T::AffidavitCrypto as AppCrypto<T::Public, T::Signature>>::RuntimeAppPublic
                as RuntimeAppPublic>::all();

        // Fast path: no keys exist in the keystore
        if all_keys.is_empty() {
            return Ok(());
        }

        // Check whether the active affidavit key is already available for signing
        for key in all_keys.into_iter() {
            let generic_pub:
                <T::AffidavitCrypto as AppCrypto<T::Public, T::Signature>>::GenericPublic =
                key.into();
            let public: T::Public = generic_pub.into();
            let account: AffidavitId<T> = public.clone().into_account().into();

            if account == afdt_key {
                // Active affidavit key already exists and is usable
                return Err(<Self as Logging<BlockNumberFor<T>>>::debug(
                    &Error::<T>::AffidavitKeyExists.into(),
                    self.at,
                    LOG_TARGET_AFDT,
                    Some(std_fmt::<T>),
                ));
            }
        }

        Ok(())
    }

    /// Initializes a new affidavit key pair and marks it as the
    /// **active affidavit key** for the node.
    ///
    /// This routine:
    /// 1. Generates a new affidavit application key pair in the local keystore.
    /// 2. Extracts the public key and derives its corresponding `AffidavitId`.
    /// 3. Stores the public identifier as the active affidavit key in
    ///    the offchain storage.
    ///
    /// Affidavit keys are **rotated regularly** (e.g. per session). The node's
    /// long-term authority or author role i.e., stash key is never used directly
    /// for affidavit signing, reducing operational risk.
    ///
    /// If storing the active affidavit key fails, the generated key pair
    /// becomes unreachable and is effectively discarded. The routine will
    /// retry on subsequent block OCW executions until initialization succeeds.
    fn run_service(&self) -> Result<(), Self::Logger> {
        if let Err(e) = Self::can_run(&self) {
            // Fast Path if initialization is not required
            if e == Error::<T>::AffidavitKeyExists.into() {
                return Ok(());
            }
            return Err(e);
        }

        // Generate a new affidavit application key pair
        let key =
            <<T::AffidavitCrypto as AppCrypto<T::Public, T::Signature>>::RuntimeAppPublic
                as RuntimeAppPublic>::generate_pair(None);

        // Raw conversions
        let generic_pub: <T::AffidavitCrypto as AppCrypto<T::Public, T::Signature>>::GenericPublic =
            key.into();
        let public: T::Public = generic_pub.into();
        let account: AffidavitId<T> = public.clone().into_account().into();

        // Stabilize the public identifier as the active affidavit key
        if Finalized::<T, AffidavitId<T>, Self, Pallet<T>>::insert(
            ACTIVE_AFDT_KEY,
            &account,
            LOG_TARGET_AFDT,
            None,
        )
        .is_err()
        {
            let block = self.at;
            return Err(<Self as Logging<BlockNumberFor<T>>>::warn(
                &Error::<T>::SetNewAffidavitKeyFailed.into(),
                block,
                LOG_TARGET_AFDT,
                Some(std_fmt::<T>),
            ));
        }

        Ok(())
    }

    /// Logs a `info` message on a successful [`InitAffidavitKey`] routine.
    fn on_ran_service(&self) {
        <Self as Logging<BlockNumberFor<T>>>::debug(
            &Error::<T>::InitAffidavitKeyRoutineSuccess.into(),
            self.at,
            LOG_TARGET_AFDT,
            Some(std_fmt::<T>),
        );
    }
}

// ===============================================================================
// ````````````````````````````` TRY ELECTION (OCW) ``````````````````````````````
// ===============================================================================

// `TryElection` reuses `DeclareAffidavit` offchain storage for key coordination
// and state tracking, avoiding duplication.
// No additional offchain key-value storage is defined for this routine.

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// ````````````````````````````````` ROUTINE OF ``````````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

/// Authorization layer for the **election execution routine**.
///
/// This implementation resolves the **affidavit public key** authorized
/// to run the election flow. It deliberately reuses the key-resolution
/// logic from [`DeclareAffidavit`] to enforce strict lifecycle ordering:
///
/// ```text
/// Affidavit Declaration -> Key Rotation -> Election
/// ```
///
/// By delegating authorization instead of duplicating it, this layer
/// guarantees that elections are executed only by authors who have
/// successfully completed the affidavit and key-rotation process.
impl<T: Config> RoutineOf<T::Public, BlockNumberFor<T>> for TryElection<T> {
    /// Determines the affidavit public key authorized to run the election routine.
    ///
    /// ## Semantics
    /// - Reuses the **currently active affidavit key** resolved by
    ///   [`DeclareAffidavit`].
    /// - Ensures that election execution is authorized by the same
    ///   operational key that is eligible for affidavit declaration.
    ///
    /// ## Rationale
    /// Elections are permitted **only after** a successful affidavit
    /// declaration and key-rotation cycle. By delegating to
    /// `DeclareAffidavit::who`, this routine enforces a strict
    /// lifecycle ordering without duplicating key-resolution logic.
    ///
    /// ## Key Resolution Note
    /// Although this appears syntactically as the *active affidavit key*,
    /// it semantically represents the **recently rotated next affidavit key**
    /// that was promoted to active status by the previous OCW execution's
    /// final routine ([`RotateAffidavitKey`]).
    fn who(at: &BlockNumberFor<T>) -> Result<T::Public, Self::Logger> {
        // Returns only if the Active Affidavit Key is Initialized, Stored and Finalized
        let who = <DeclareAffidavit<T> as RoutineOf<T::Public, BlockNumberFor<T>>>::who(at)?;
        Ok(who)
    }
}

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// `````````````````````````````````` ROUTINES ```````````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

/// Offchain routine responsible for **opportunistic election execution**.
///
/// This routine attempts to submit the `elect` extrinsic for the
/// upcoming session using the **currently active affidavit key**.
///
/// It is intentionally designed to be:
/// - **Non-blocking**: exits silently when prerequisites are unmet.
/// - **Idempotent**: safe to run repeatedly across blocks and forks.
/// - **Pre-emptive**: elections may be attempted before the OCW pipeline
///   fully converges, succeeding in later executions.
///
/// The routine participates in the OCW execution pipeline:
///
/// ```text
/// InitAffidavitKey -> TryElection -> DeclareAffidavit -> RotateAffidavitKey
/// ```
///
/// and relies on repeated OCW invocations to eventually satisfy all
/// temporal and state-dependent constraints.
impl<T: Config> Routines<BlockNumberFor<T>> for TryElection<T> {
    /// Checks whether an election can be processed in the current block.
    ///
    /// ## Semantics
    /// - Delegates election window validation to
    ///   [`ElectAuthors::can_process_election`].
    /// - Does **not** treat an unavailable election window as an error.
    ///
    /// ## Behavior
    /// - If the election window is not open, the routine exits early
    ///   with an informational log.
    /// - Hard failures (e.g. invalid configuration) are surfaced
    ///   as logged errors.
    ///
    /// This design allows the OCW orchestrator to continue executing
    /// subsequent routines (e.g. affidavit declaration) in the same block.
    fn can_run(&self) -> Result<(), Self::Logger> {
        if let Err(e) =
            <Internals<T> as ElectAuthors<AuthorOf<T>, ElectionVia<T>>>::can_process_election(&None)
        {
            return Err(<Self as Logging<BlockNumberFor<T>>>::debug(
                &e,
                self.at,
                LOG_TARGET_ELEC,
                Some(std_fmt::<T>),
            ));
        }
        Ok(())
    }

    /// Attempts to submit the election transaction for the upcoming session.
    ///
    /// ## Execution Strategy
    /// This routine is **opportunistic and non-blocking**:
    ///
    /// - If the election window is not open, execution passes silently to next routine.
    /// - If the author is not eligible, execution exits silently.
    /// - If the author already acted as the election runner, execution exits silently.
    ///
    /// This behavior is intentional and allows the OCW pipeline to be
    /// orchestrated in the following order:
    ///
    /// ```text
    /// InitAffidavitKey -> TryElection -> DeclareAffidavit -> RotateAffidavitKey
    /// ```
    ///
    /// Elections are therefore attempted *pre-emptively* and may succeed
    /// in a later OCW invocation once all prerequisites converge.
    ///
    /// ## Semantics
    /// - Uses the **currently active affidavit key** to authorize the election.
    /// - Submits an unsigned `elect` extrinsic signed offchain.
    /// - Ensures that each author runs the election at most once per session.
    ///
    /// ## Failure Handling
    /// - All signing or submission failures are logged as errors.
    /// - No retries are attempted in the same block.
    /// - Subsequent OCW executions may retry automatically.
    fn run_service(&self) -> Result<(), Self::Logger> {
        // Exit early if election window is unavailable
        // And try the next routine of declaring affidavit
        if let Err(_) = Self::can_run(&self) {
            return Ok(());
        }

        let for_session = CurrentSession::<T>::get().saturating_add(One::one());

        // Resolve author from affidavit key registered for election (session + 2)
        let afdt_pub: AffidavitId<T> = self.by.clone().into_account().into();
        let Some(author) =
            AffidavitKeys::<T>::get((for_session.saturating_add(One::one()), afdt_pub))
        else {
            // The active affidavit key is not for elections
            // but for declaring affidavit
            <Self as Logging<BlockNumberFor<T>>>::debug(
                &Error::<T>::AffidavitKeyForDeclaration.into(),
                self.at,
                LOG_TARGET_ELEC,
                Some(std_fmt::<T>),
            );
            return Ok(());
        };

        // Prevent duplicate election execution by the same author
        // Until this reflects, repeated OCW executions may submit
        // duplicate transactions which is expected to be filtered by
        // `ValidateUnsigned`'s `ValidTransaction::and_provides()`
        if let Some((runner, _)) = ElectsPreparedBy::<T>::get(for_session) {
            if author == runner {
                // Already ran election
                // Try declaring affidavit during the upcoming session
                return Ok(());
            }
        }

        let payload = ElectionPayload {
            public: self.by.clone(),
        };

        // Sign election payload using affidavit key
        let Some(signature) =
            <ElectionPayload<T::Public> as SignedPayload<T>>::sign::<T::AffidavitCrypto>(&payload)
        else {
            return Err(<Self as Logging<BlockNumberFor<T>>>::error(
                &Error::<T>::CannotSignElectionTxPayload.into(),
                self.at,
                LOG_TARGET_ELEC,
                Some(std_fmt::<T>),
            ));
        };

        // Submit election extrinsic
        let signer = Signer::<T, T::AffidavitCrypto>::any_account();
        let result = signer.send_signed_transaction(|_| {
            <T as crate::Config>::RuntimeCall::from(crate::Call::elect {
                payload: payload.clone(),
                signature: signature.clone(),
            })
            .into()
        });

        match result {
            Some((_, Ok(_))) => Ok(()),

            Some((_, Err(_))) => Err(<Self as Logging<BlockNumberFor<T>>>::error(
                &Error::<T>::ExtrinsicFailedToElectAuthors.into(),
                self.at,
                LOG_TARGET_ELEC,
                Some(std_fmt::<T>),
            )),

            None => Err(<Self as Logging<BlockNumberFor<T>>>::error(
                &Error::<T>::CannotSubmitElectionTx.into(),
                self.at,
                LOG_TARGET_ELEC,
                Some(std_fmt::<T>),
            )),
        }
    }

    /// Logs a `info` message on a successful [`TryElection`] routine.
    fn on_ran_service(&self) {
        <Self as Logging<BlockNumberFor<T>>>::debug(
            &Error::<T>::TryElectionRoutineSuccess.into(),
            self.at,
            LOG_TARGET_ELEC,
            Some(std_fmt::<T>),
        );
    }
}

// ===============================================================================
// ``````````````````````````` DECLARE AFFIDAVIT (OCW) ```````````````````````````
// ===============================================================================

// --- Keys ---

/// Offchain storage key identifying the **next affidavit key**.
///
/// This key represents the affidavit public identifier that:
/// - is declared during the current affidavit window,
/// - undergoes finality evaluation via [`Finalized`] storage semantics,
/// - and is later promoted to the **active affidavit key**
///   by the [`RotateAffidavitKey`] routine.
///
/// The value stored under this key is **session-forward looking** and
/// must survive fork re-orgs and confidence evaluation before it is
/// considered safe for activation.
pub const NEXT_AFDT_KEY: &'static [u8] = b"NEXT_AFDT_KEY";

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// ``````````````````````````````` ERROR PROVIDERS ```````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

/// Error policy for **fork-aware speculative storage** of the next affidavit key.
///
/// This implementation defines how storage-layer failures originating
/// from fork-aware offchain storage are surfaced as
/// routine-specific [`DispatchError`](sp_runtime::DispatchError) values.
///
/// These errors relate exclusively to the **speculative identity**
/// (`ValueHash`) used to track the next affidavit key across forks.
///
/// Any failure reported here:
/// - is logged exactly once by the storage layer,
/// - and returned unchanged to the caller.
impl<T: Config> OffchainStorageError<ForkAware<T, ValueHash, DeclareAffidavit<T>, Pallet<T>>>
    for DeclareAffidavit<T>
{
    type Error = Error<T>;

    /// Returned when decoding the speculative hash fails.
    ///
    /// Indicates corrupted or unexpected fork-local storage state.
    fn decode_failed() -> Self::Error {
        Error::<T>::NextAfdtKeySpeculativeHashDecodeFail
    }

    /// Returned when a concurrent mutation of the speculative hash is detected.
    ///
    /// Indicates overlapping OCW executions or race conditions.
    fn concurrent_mutation() -> Self::Error {
        Error::<T>::NextAfdtKeySpeculativeHashConcurrentMutation
    }
}

/// Error policy for **persistent finalized storage** of the next affidavit key.
///
/// This implementation defines error signaling for failures that occur
/// while interacting with the persistent observation ledger backing
/// the [`Finalized`] storage model.
///
/// These errors concern the **finalized value and its observation history**,
/// not speculative fork-local state.
impl<T: Config> OffchainStorageError<Persistent<T, Ledger<T, AffidavitId<T>>, DeclareAffidavit<T>>>
    for DeclareAffidavit<T>
{
    type Error = Error<T>;

    /// Returned when decoding the persistent ledger entry fails.
    ///
    /// Indicates corrupted or incompatible persisted offchain data.
    fn decode_failed() -> Self::Error {
        Error::<T>::NextAfdtKeyFinalizedValueDecodeFail
    }

    /// Returned when a concurrent mutation of the persistent ledger is detected.
    ///
    /// Indicates contention between multiple OCW executions.
    fn concurrent_mutation() -> Self::Error {
        Error::<T>::NextAfdtKeyFinalizedValueConcurrentMutation
    }
}

/// Finality-specific invariant errors for the next affidavit key.
///
/// These errors are emitted when semantic inconsistencies are detected
/// between fork-aware and persistent storage layers.
///
/// Such conditions indicate **partial or invalid state**, and the
/// storage layer automatically performs cleanup before returning
/// these errors.
impl<T: Config> FinalizedOffchainStorageError<T, AffidavitId<T>> for DeclareAffidavit<T> {
    type Error = Error<T>;

    /// Emitted when a speculative fork-aware hash exists
    /// without a corresponding persistent ledger entry.
    fn hanging_hash() -> Self::Error {
        Error::<T>::NextAfdtKeySpeculativeHangingHash
    }

    /// Emitted when **persistent storage contains no value** corresponding to
    /// an existing speculative (fork-aware) hash.
    fn hanging_value() -> Self::Error {
        Error::<T>::NextAfdtKeyFinalizedHangingValue
    }
}

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// `````````````````````````````` FINALIZED POLICY ```````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

/// Finality evaluation policy for the **next affidavit key**.
///
/// This implementation **reuses the finality parameters defined by
/// [`InitAffidavitKey`]**, ensuring that both the *active* and *next*
/// affidavit keys are evaluated under **identical confidence guarantees**.
///
/// ## Design Rationale
///
/// Every routine that uses [`Finalized`] storage:
/// - must define exactly **one finality policy**, and
/// - that policy is **scoped to the routine**, not to the storage backend.
///
/// Since the *next affidavit key* participates in the **same operational
/// lifecycle** as the active affidavit key (generation -> observation -> rotation),
/// it is intentionally governed by the same time- and observation-based
/// confidence thresholds.
///
/// ## Invariants
///
/// - [`Finalized`] storage is **strictly constrained**:
///   - one logical value per routine,
///   - one finality policy per value.
/// - Unlike [`ForkAware`] or [`Persistent`] storage backends, [`Finalized`]
///   does **not** allow multiple independent values or heterogeneous policies.
///
/// Reusing the policy from [`InitAffidavitKey`] preserves these invariants
/// while avoiding duplicated configuration and potential divergence.
impl<T: Config> FinalizedPolicy<T> for DeclareAffidavit<T> {
    /// Wall-clock delay required before confidence evaluation begins.
    fn finality_after() -> <T as pallet_timestamp::Config>::Moment {
        <InitAffidavitKey<T> as FinalizedPolicy<T>>::finality_after()
    }

    /// Number of block-scoped observations required to reach strong confidence.
    fn finality_ticks() -> BlockNumberFor<T> {
        <InitAffidavitKey<T> as FinalizedPolicy<T>>::finality_ticks()
    }
}

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// `````````````````````````````````` ROUTINE OF `````````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

/// Declares the authorization requirements for the [`DeclareAffidavit`] routine.
///
/// This implementation specifies **who is permitted to execute** the routine
/// at a given point in time.
///
/// The routine is restricted to the node's **active affidavit key**. This
/// ensures that affidavit declarations are signed only by the currently
/// designated, rotated operational key, rather than by long-term authority
/// or stash keys.
///
/// The returned `T::Public` value represents the concrete public key that
/// must be used to sign the affidavit payload at the given block number.
impl<T: Config> RoutineOf<T::Public, BlockNumberFor<T>> for DeclareAffidavit<T> {
    /// Determines which public key is authorized to execute this routine.
    ///
    /// ## Why this check exists
    ///
    /// Affidavit declarations are security-sensitive operations. To reduce
    /// the blast radius of key compromise and to support regular key rotation,
    /// only the **currently active affidavit key** is permitted to authorize
    /// this routine.
    ///
    /// The active affidavit key:
    /// - is stored and ensured by [`Finalized`] offchain storage,
    /// - is expected to have a corresponding key pair in the node's keystore,
    /// - and must be explicitly initialized before this routine can run.
    ///
    /// ## Failure semantics
    ///
    /// - Any storage inconsistency is treated as a **hard error**, since it
    ///   indicates corrupted or unexpected node state.
    /// - If no active affidavit key is configured, the caller is considered
    ///   misconfigured and execution is refused.
    /// - If the active affidavit key exists but the corresponding key pair
    ///   is missing from the keystore, execution is refused.
    fn who(at: &BlockNumberFor<T>) -> Result<T::Public, Self::Logger> {
        // Initialized by `InitAffidavitKey`; reused here to avoid duplicating implementations
        let result = Finalized::<T, AffidavitId<T>, InitAffidavitKey<T>, Pallet<T>>::get(
            ACTIVE_AFDT_KEY,
            LOG_TARGET_AFDT,
            None,
        );

        let afdt_key = match result {
            Ok(Some(Confidence::Safe(key))) => key,
            Ok(None) => {
                return Err(<Self as Logging<BlockNumberFor<T>>>::error(
                    &Error::<T>::ExpectedToHoldActiveAffidavitKey.into(),
                    *at,
                    LOG_TARGET_AFDT,
                    Some(std_fmt::<T>),
                ))
            }
            Ok(Some(_)) => {
                return Err(<Self as Logging<BlockNumberFor<T>>>::debug(
                    &Error::<T>::ActiveAfdtKeyNotYetFinalized.into(),
                    *at,
                    LOG_TARGET_AFDT,
                    Some(std_fmt::<T>),
                ))
            }
            Err(_) => {
                return Err(<Self as Logging<BlockNumberFor<T>>>::warn(
                    &Error::<T>::OCWStorageDecisionHalt.into(),
                    *at,
                    LOG_TARGET_AFDT,
                    Some(std_fmt::<T>),
                ));
            }
        };

        // Retrieve all affidavit keys currently available in the local keystore
        let all_keys =
            <<T::AffidavitCrypto as AppCrypto<T::Public, T::Signature>>::RuntimeAppPublic
                as RuntimeAppPublic>::all();

        // Ensure the active affidavit key has a corresponding key pair
        for key in all_keys.into_iter() {
            let generic_pub:
                <T::AffidavitCrypto as AppCrypto<T::Public, T::Signature>>::GenericPublic =
                key.into();
            let public: T::Public = generic_pub.into();
            let account: AffidavitId<T> = public.clone().into_account().into();

            if account == afdt_key {
                // Authorized signer found
                return Ok(public);
            }
        }

        // Active affidavit key exists but is not usable for signing
        Err(<Self as Logging<BlockNumberFor<T>>>::error(
            &Error::<T>::ExpectedActiveAffidavitKeyPairNotFound.into(),
            *at,
            LOG_TARGET_AFDT,
            Some(std_fmt::<T>),
        ))
    }
}

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// ``````````````````````````````````` ROUTINES ``````````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

/// Offchain routine responsible for **affidavit declaration** and
/// **preparing key rotation for the next election cycle**.
///
/// ## What an affidavit represents
///
/// Declaring an affidavit signals that an author:
/// - is **ready to participate in the upcoming election**, and
/// - satisfies all protocol-defined requirements (e.g. backing, support).
///
/// An affidavit:
/// - is submitted via an **unsigned extrinsic**,
/// - is allowed **only within a bounded affidavit window**, and
/// - is restricted to **valid authors** holding the correct operational role.
///
/// ## Responsibilities of this routine
///
/// This OCW routine performs the **offchain coordination** required to:
///
/// 1. Verify that affidavit submission is currently permitted.
/// 2. Ensure a **next affidavit key** exists and is finalized.
/// 3. Submit the `declare` extrinsic using the **active affidavit key**.
/// 4. Attach the **next affidavit key** for rotation into the upcoming session.
///
/// ## Key separation and security
///
/// - Long-term authority / stash keys are **never used** here.
/// - All signing is performed using **ephemeral affidavit keys**
///   managed via application crypto and rotated regularly.
/// - This minimizes the blast radius of key compromise and
///   enables strict lifecycle enforcement.
///
/// ## Execution model
///
/// This routine is designed to be **optimistic and non-blocking**:
///
/// - It may be executed before the node has fully completed validation
/// via `validate` extrinsic.
/// - Runtime-side checks ultimately decide whether the extrinsic succeeds.
/// - Failures are logged and retried automatically in later OCW executions.
///
/// This makes the routine safe to run repeatedly without risking
/// inconsistent on-chain state.
impl<T: Config> Routines<BlockNumberFor<T>> for DeclareAffidavit<T> {
    /// Determines whether the affidavit declaration routine may proceed.
    ///
    /// ## Semantics
    ///
    /// - Ensures that a **next affidavit key** exists in finalized offchain
    ///   storage, else initiates and finalizes one.
    /// - Verifies that the **current affidavit key** is eligible to submit
    ///   an affidavit in the runtime.
    ///
    /// ## Key handling
    ///
    /// - The active affidavit key (`self.by`) **must already exist** in the
    ///   node's keystore.
    /// - Author primary keys are **never consulted**.
    ///
    /// ## Optimistic execution
    ///
    /// This function may be called **before** the author has successfully
    /// executed the on-chain `validate`.
    ///
    /// That is safe because:
    /// - Runtime-side checks enforce correctness.
    /// - This routine merely prepares and submits the transaction.
    ///
    /// If the author has not yet validated their affidavit key on-chain,
    /// the submission will be rejected harmlessly.
    fn can_run(&self) -> Result<(), Self::Logger> {
        let afdt_pub = &self.by;
        let block = self.at;

        // Ensure a next affidavit key exists (or create one)
        let result =
            Finalized::<T, AffidavitId<T>, Self, Pallet<T>>::get(NEXT_AFDT_KEY, LOG_TARGET_AFDT, None);

        match result {
            // No next key yet -> generate and persist one
            Ok(None) => {
                let new_key =
                    <<T::AffidavitCrypto as AppCrypto<T::Public, T::Signature>>::RuntimeAppPublic
                        as RuntimeAppPublic>::generate_pair(None);

                let public: T::Public = {
                    let generic_pub:
                        <T::AffidavitCrypto as AppCrypto<T::Public, T::Signature>>::GenericPublic =
                        new_key.into();
                    generic_pub.into()
                };

                let new_afdt_key = public.clone().into_account();

                if Finalized::<T, AffidavitId<T>, Self, Pallet<T>>::insert(
                    NEXT_AFDT_KEY,
                    &new_afdt_key,
                    LOG_TARGET_AFDT,
                    None,
                )
                .is_err()
                {
                    return Err(<Self as Logging<BlockNumberFor<T>>>::warn(
                        &Error::<T>::SetNextAffidavitKeyFailed.into(),
                        block,
                        LOG_TARGET_AFDT,
                        Some(std_fmt::<T>),
                    ));
                }
                return Err(<Self as Logging<BlockNumberFor<T>>>::debug(
                    &Error::<T>::NextAfdtKeyNotYetFinalized.into(),
                    block,
                    LOG_TARGET_AFDT,
                    Some(std_fmt::<T>),
                ));
            }

            // Next key exists and finalized
            Ok(Some(Confidence::Safe(_))) => {}

            Ok(Some(_)) => {
                return Err(<Self as Logging<BlockNumberFor<T>>>::debug(
                    &Error::<T>::NextAfdtKeyNotYetFinalized.into(),
                    block,
                    LOG_TARGET_AFDT,
                    Some(std_fmt::<T>),
                ))
            }

            // Storage inconsistency -> hard stop
            Err(_) => {
                return Err(<Self as Logging<BlockNumberFor<T>>>::warn(
                    &Error::<T>::OCWStorageDecisionHalt.into(),
                    block,
                    LOG_TARGET_AFDT,
                    Some(std_fmt::<T>),
                ));
            }
        }

        // Check runtime-level affidavit eligibility
        if let Err(e) =
            <Pallet<T> as ElectionAffidavits<AffidavitId<T>, ElectionVia<T>>>::can_submit_affidavit(
                (&afdt_pub.clone().into_account()).into(),
            )
        {
            return Err(<Self as Logging<BlockNumberFor<T>>>::debug(
                &e,
                block,
                LOG_TARGET_AFDT,
                Some(std_fmt::<T>),
            ));
        }

        Ok(())
    }

    /// Submits the `declare` extrinsic.
    ///
    /// ## Preconditions
    ///
    /// - The active affidavit key is authorized and present in the keystore.
    /// - The next affidavit key exists and has reached sufficient finality.
    ///
    /// ## Behavior
    ///
    /// - Signs the affidavit payload using the **current affidavit key**.
    /// - Includes the **next affidavit key** for rotation.
    /// - Submits the extrinsic to the transaction pool.
    ///
    /// ## Important notes
    ///
    /// - This routine **does not apply any on-chain state changes itself**.
    /// - It only submits a transaction; actual effects occur later
    ///   during block execution.
    /// - Any failure is logged and retried in future OCW runs.
    fn run_service(&self) -> Result<(), Self::Logger> {
        Self::can_run(self)?;

        let afdt_key = &self.by;
        let block = self.at;

        let result =
            Finalized::<T, AffidavitId<T>, Self, Pallet<T>>::get(NEXT_AFDT_KEY, LOG_TARGET_AFDT, None);

        let next_afdt_key = match result {
            Ok(Some(Confidence::Safe(key))) => key,
            Ok(Some(_)) => {
                return Err(<Self as Logging<BlockNumberFor<T>>>::error(
                    &Error::<T>::ExpectedToHoldFinalizedNextAffidavitKey.into(),
                    block,
                    LOG_TARGET_AFDT,
                    Some(std_fmt::<T>),
                ))
            }
            Ok(None) => {
                return Err(<Self as Logging<BlockNumberFor<T>>>::error(
                    &Error::<T>::ExpectedToHoldFinalizedNextAffidavitKey.into(),
                    block,
                    LOG_TARGET_AFDT,
                    Some(std_fmt::<T>),
                ))
            }
            Err(_) => {
                return Err(<Self as Logging<BlockNumberFor<T>>>::warn(
                    &Error::<T>::OCWStorageDecisionHalt.into(),
                    block,
                    LOG_TARGET_AFDT,
                    Some(std_fmt::<T>),
                ))
            }
        };

        let payload = AffidavitPayload {
            public: afdt_key.clone(),
            rotate: next_afdt_key.clone(),
        };

        let Some(signature) =
            <AffidavitPayload<T::Public, AffidavitId<T>> as SignedPayload<T>>::sign::<
                T::AffidavitCrypto,
            >(&payload)
        else {
            return Err(<Self as Logging<BlockNumberFor<T>>>::error(
                &Error::<T>::CannotSignAffidavitTxPayload.into(),
                block,
                LOG_TARGET_AFDT,
                Some(std_fmt::<T>),
            ));
        };

        let signer = Signer::<T, T::AffidavitCrypto>::any_account();

        // If repeated OCW executions, this may result in duplicate transactions which
        // shall be filtered out by `ValidateUnsigned`'s `ValidTransaction::and_provides()`
        let result = signer.send_signed_transaction(|_| {
            <T as crate::Config>::RuntimeCall::from(crate::Call::<T>::declare {
                payload: payload.clone(),
                signature: signature.clone(),
            })
            .into()
        });

        match result {
            Some((_, Ok(_))) => Ok(()),

            Some((_, Err(_))) => Err(<Self as Logging<BlockNumberFor<T>>>::error(
                &Error::<T>::FailedToDeclareAffidavit.into(),
                block,
                LOG_TARGET_AFDT,
                Some(std_fmt::<T>),
            )),

            None => Err(<Self as Logging<BlockNumberFor<T>>>::error(
                &Error::<T>::CannotSubmitAffidavitTx.into(),
                block,
                LOG_TARGET_AFDT,
                Some(std_fmt::<T>),
            )),
        }
    }

    /// Logs a `info` message on a successful [`DeclareAffidavit`] routine.
    fn on_ran_service(&self) {
        <Self as Logging<BlockNumberFor<T>>>::debug(
            &Error::<T>::DeclarAffidavitRoutineSuccess.into(),
            self.at,
            LOG_TARGET_AFDT,
            Some(std_fmt::<T>),
        );
    }
}
// ===============================================================================
// ````````````````````````` ROTATE AFFIDAVIT KEY (OCW) ``````````````````````````
// ===============================================================================

// `RotateAffidavitKey` reuses `DeclareAffidavit` and `InitAffidavitKey` offchain
// storage for key coordination and state tracking, avoiding duplication.
// No additional offchain key-value storage is defined for this routine.

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// ````````````````````````````````` ROUTINE OF ``````````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

/// Authorization layer for affidavit key rotation.
///
/// Resolves the **finalized next affidavit key** prepared by
/// [`DeclareAffidavit`] and ensures it is locally usable for signing.
///
/// This enforces that rotation is performed only with a key that:
/// - has reached finality, and
/// - exists in the node's keystore.
impl<T: Config> RoutineOf<T::Public, BlockNumberFor<T>> for RotateAffidavitKey<T> {
    /// Determines the next affidavit public key authorized to **finalize key rotation**.
    ///
    /// ## Semantics
    ///
    /// - Resolves the **next affidavit key** previously prepared and finalized
    ///   by the [`DeclareAffidavit`] routine.
    /// - Requires the key to be in a [`Confidence::Safe`] state, ensuring it has
    ///   survived fork re-orgs and met finality requirements.
    ///
    /// ## Lifecycle position
    ///
    /// Reaching this routine implies:
    ///
    /// - The `declare` extrinsic has already been **submitted**, and
    /// - The node is now waiting to observe whether that transaction has been
    ///   **accepted and reflected in runtime storage**.
    ///
    /// This routine therefore does **not generate or mutate keys**. It only:
    /// - re-reads the finalized *next* affidavit key, and
    /// - verifies that the corresponding key pair still exists in the local keystore.
    ///
    /// ## Failure semantics
    ///
    /// - Missing or non-finalized next affidavit key is treated as a hard stop.
    /// - Storage inconsistencies indicate OCW coordination failure and halt execution.
    /// - If the key exists in storage but not in the keystore, the node is considered
    ///   misconfigured.
    fn who(at: &BlockNumberFor<T>) -> Result<T::Public, Self::Logger> {
        let result = Finalized::<T, AffidavitId<T>, DeclareAffidavit<T>, Pallet<T>>::get(
            NEXT_AFDT_KEY,
            LOG_TARGET_AFDT,
            None,
        );

        let afdt_key = match result {
            Ok(Some(Confidence::Safe(key))) => key,

            Ok(None) => {
                return Err(<Self as Logging<BlockNumberFor<T>>>::error(
                    &Error::<T>::ExpectedToHoldFinalizedNextAffidavitKey.into(),
                    *at,
                    LOG_TARGET_AFDT,
                    Some(std_fmt::<T>),
                ))
            }

            Ok(Some(_)) => {
                return Err(<Self as Logging<BlockNumberFor<T>>>::info(
                    &Error::<T>::ExpectedToHoldFinalizedNextAffidavitKey.into(),
                    *at,
                    LOG_TARGET_AFDT,
                    Some(std_fmt::<T>),
                ))
            }

            Err(_) => {
                return Err(<Self as Logging<BlockNumberFor<T>>>::warn(
                    &Error::<T>::OCWStorageDecisionHalt.into(),
                    *at,
                    LOG_TARGET_AFDT,
                    Some(std_fmt::<T>),
                ));
            }
        };

        // Ensure the finalized next affidavit key exists in the local keystore
        let all_keys =
            <<T::AffidavitCrypto as AppCrypto<T::Public, T::Signature>>::RuntimeAppPublic
                as RuntimeAppPublic>::all();

        for key in all_keys {
            let generic_pub:
                <T::AffidavitCrypto as AppCrypto<T::Public, T::Signature>>::GenericPublic =
                key.into();
            let public: T::Public = generic_pub.into();
            let account: AffidavitId<T> = public.clone().into_account().into();

            if account == afdt_key {
                return Ok(public);
            }
        }

        // Finalized key exists, but no signing key is available locally
        Err(<Self as Logging<BlockNumberFor<T>>>::warn(
            &Error::<T>::ExpectedNextAffidavitKeyPairNotFound.into(),
            *at,
            LOG_TARGET_AFDT,
            Some(std_fmt::<T>),
        ))
    }
}

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// `````````````````````````````````` ROUTINES ```````````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

/// Offchain routine responsible for **finalizing affidavit key rotation**.
///
/// Completes the lifecycle by promoting the finalized next affidavit key
/// to active status once the runtime reflects successful affidavit submission.
///
/// This routine:
/// - waits for on-chain confirmation of the rotated key,
/// - performs a safe transition from next -> active key,
/// - resets state on failure to avoid inconsistent lifecycle progression.
///
/// Designed to be idempotent and driven by repeated OCW execution.
impl<T: Config> Routines<BlockNumberFor<T>> for RotateAffidavitKey<T> {
    /// Determines whether affidavit key rotation may be finalized.
    ///
    /// ## Semantics
    ///
    /// - Checks whether the **next affidavit key** has been successfully
    ///   registered in runtime storage for the **session after next**
    ///   (`current_session + 2`) (for which next election will be conducted).
    /// - Presence of this key in [`AffidavitKeys`] is treated as confirmation
    ///   that the `declare` extrinsic was accepted.
    ///
    /// ## Waiting behavior
    ///
    /// - If the key is not yet visible, the routine waits passively.
    /// - While the affidavit window is still open, this is logged as
    ///   an informational "awaiting status" condition.
    ///
    /// ## Failure and recovery
    ///
    /// If the affidavit window has **closed** and:
    /// - the next affidavit key is still not registered, then
    /// - the system assumes the affidavit transaction failed or was dropped.
    ///
    /// In that case:
    /// - All affidavit-related offchain state is cleared.
    /// - Active validation is considered stopped.
    /// - The node must restart the lifecycle via `validate` extrinsic in a later block.
    ///
    /// This aggressive reset prevents the OCW from getting stuck
    /// in a half-rotated or inconsistent state.
    fn can_run(&self) -> Result<(), Self::Logger> {
        let current_session = CurrentSession::<T>::get();
        let for_session = current_session.saturating_add(2);
        let next_afdt_pub: AffidavitId<T> = self.by.clone().into_account().into();

        if !AffidavitKeys::<T>::contains_key((for_session, next_afdt_pub)) {
            // Determine end of session
            let session_start = SessionStartAt::<T>::get();
            let avg_session_len =
                <<T as crate::Config>::NextSessionRotation as EstimateNextSessionRotation<
                    BlockNumberFor<T>,
                >>::average_session_length();
            let end_block = session_start.saturating_add(avg_session_len);

            // If window expired, perform hard recovery
            if self.at >= end_block {
                Finalized::<T, AffidavitId<T>, DeclareAffidavit<T>, Pallet<T>>::remove(
                    NEXT_AFDT_KEY,
                    LOG_TARGET_AFDT,
                    None,
                )
                .map_err(|_| {
                    <Self as Logging<BlockNumberFor<T>>>::warn(
                        &Error::<T>::OCWStorageDecisionHalt.into(),
                        self.at,
                        LOG_TARGET_AFDT,
                        Some(std_fmt::<T>),
                    )
                })?;

                Finalized::<T, AffidavitId<T>, InitAffidavitKey<T>, Pallet<T>>::remove(
                    ACTIVE_AFDT_KEY,
                    LOG_TARGET_AFDT,
                    None,
                )
                .map_err(|_| {
                    <Self as Logging<BlockNumberFor<T>>>::warn(
                        &Error::<T>::OCWStorageDecisionHalt.into(),
                        self.at,
                        LOG_TARGET_AFDT,
                        Some(std_fmt::<T>),
                    )
                })?;

                return Err(<T as Logging<BlockNumberFor<T>>>::error(
                    &Error::<T>::ValidationStopped.into(),
                    self.at,
                    LOG_TARGET_AFDT,
                    Some(std_fmt::<T>),
                ));
            }

            // Still waiting for runtime reflection
            return Err(<T as Logging<BlockNumberFor<T>>>::debug(
                &Error::<T>::AffidavitTxAwaitingStatus.into(),
                self.at,
                LOG_TARGET_AFDT,
                Some(std_fmt::<T>),
            ));
        }

        Ok(())
    }

    /// Finalizes affidavit key rotation after successful affidavit acceptance.
    ///
    /// ## Behavior
    ///
    /// Once the runtime reflects the rotated affidavit key:
    ///
    /// 1. The **next affidavit key** is removed from finalized offchain storage.
    /// 2. The **active affidavit key** is updated to the rotated key.
    ///
    /// This completes the affidavit lifecycle for the current session and
    /// enables subsequent OCW executions to:
    /// - attempt elections via [`TryElection`], and
    /// - later begin a new affidavit cycle.
    ///
    /// ## Guarantees
    ///
    /// - Rotation is performed **exactly once** per successful affidavit.
    /// - Any storage failure is treated as a coordination halt.
    fn run_service(&self) -> Result<(), Self::Logger> {
        Self::can_run(self)?;

        Finalized::<T, AffidavitId<T>, DeclareAffidavit<T>, Pallet<T>>::remove(
            NEXT_AFDT_KEY,
            LOG_TARGET_AFDT,
            None,
        )
        .map_err(|_| {
            <Self as Logging<BlockNumberFor<T>>>::warn(
                &Error::<T>::OCWStorageDecisionHalt.into(),
                self.at,
                LOG_TARGET_AFDT,
                Some(std_fmt::<T>),
            )
        })?;

        Finalized::<T, AffidavitId<T>, InitAffidavitKey<T>, Pallet<T>>::mutate(
            ACTIVE_AFDT_KEY,
            |_| Ok(self.by.clone().into_account().into()),
            LOG_TARGET_AFDT,
            None,
        )
        .map_err(|_| {
            <Self as Logging<BlockNumberFor<T>>>::warn(
                &Error::<T>::OCWStorageDecisionHalt.into(),
                self.at,
                LOG_TARGET_AFDT,
                Some(std_fmt::<T>),
            )
        })?;

        Ok(())
    }

    /// Logs a `info` message on a successful [`RotateAffidavitKey`] routine.
    fn on_ran_service(&self) {
        <Self as Logging<BlockNumberFor<T>>>::debug(
            &Error::<T>::RotateAffidavitKeyRoutineSuccess.into(),
            self.at,
            LOG_TARGET_AFDT,
            Some(std_fmt::<T>),
        );
    }
}

// ===============================================================================
// ``````````````````````````````` ROUTINES HANDLER ``````````````````````````````
// ===============================================================================

/// Implements the fork graph management for this pallet's offchain worker.
///
/// Binds the pallet to the [`ForksHandler`] trait using [`ForkLocalDepot`]
/// as the scope type, wiring fork graph constants and error variants defined
/// in [`Config`] and [`Error`] into the fork resolution and recovery logic.
///
/// This impl is the prerequisite for calling [`ForksHandler::start`] inside
/// [`Hooks::offchain_worker`](frame_support::traits::Hooks::offchain_worker), 
/// which resolves the current fork branch before any routine executes.
impl<T: Config> ForksHandler<T, ForkLocalDepot> for Pallet<T> {
    const TAG: &[u8] = b"pallet_chain_manager";

    const MAX_FORKS: u32 = T::MAX_FORKS;

    const MAX_RECOVER_TRAVERSAL: u32 = T::MAX_FORK_RECOVERY_TRAVERSAL;

    fn max_forks_error() -> DispatchError {
        Error::<T>::MaxOCWForksAttained.into()
    }
    
    fn forks_not_enabled() -> DispatchError {
        Error::<T>::OCWForksNotEnabled.into()
    }
    
    fn inconsistent_forks() -> DispatchError {
        Error::<T>::OCWForksInconsistent.into()
    }
}

// ===============================================================================
// ```````````````````````````````` ROUTINES TESTS ```````````````````````````````
// ===============================================================================

#[cfg(test)]
pub mod tests {

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````````` IMPORTS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    // --- Local crate imports ---
    use crate::mock::*;

    // --- Scale-codec crates ---
    use codec::Encode;

    // --- FRAME Benchmarking ---
    use frame_benchmarking::account;

    // --- FRAME Suite ---
    use frame_suite::routines::*;

    // --- FRAME Support ---
    use frame_support::{assert_err, assert_ok, traits::{EstimateNextSessionRotation}};

    // --- Substrate primitives ---
    use sp_core::blake2_256;
    
    // --- std ---
    use std::collections::BTreeMap;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // `````````````````````````````` INIT-AFFIDAVIT-KEY `````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn init_affidavit_key_can_run_return_err_affidavit_key_exists() {
        let mut env = new_ocw_env();
        env.ext.execute_with(|| {
            init_fork_graph();
            let aff_id = generate_affidavit_id();
            insert_active_afdt_key(aff_id).unwrap();
            run_to_block(20);
            let routine = InitAffidavitKey { at: 20 };

            assert_err!(routine.can_run(), Error::AffidavitKeyExists);
        })
    }

    #[test]
    fn init_affidavit_key_can_run_return_err_active_afdt_key_not_yet_finalized() {
        let mut env = new_ocw_env();
        env.ext.execute_with(|| {
            init_fork_graph();
            let aff_id = generate_affidavit_id();
            insert_active_afdt_key(aff_id).unwrap();
            run_to_block(10);
            let routine = InitAffidavitKey { at: 10 };
            assert_err!(routine.can_run(), Error::ActiveAfdtKeyNotYetFinalized);
        })
    }

    #[test]
    fn init_affidavit_key_can_run_return_ok_since_aff_key_dose_not_exists_in_keystore() {
        let mut env = new_ocw_env();
        env.ext.execute_with(|| {
            init_fork_graph();
            let aff_id: AffidavitId = account("dummy", 1, 1);
            insert_active_afdt_key(aff_id).unwrap();
            run_to_block(20);
            let routine = InitAffidavitKey { at: 20 };
            assert_ok!(routine.can_run());
        })
    }

    #[test]
    fn init_affidavit_key_run_service_returns_ok_since_affidavit_key_already_exists() {
        let mut env = new_ocw_env();
        env.ext.execute_with(|| {
            init_fork_graph();
            let aff_id = generate_affidavit_id();
            insert_active_afdt_key(aff_id).unwrap();
            run_to_block(20);
            let routine = InitAffidavitKey { at: 20 };
            assert_ok!(routine.run_service());
        })
    }

    #[test]
    fn init_affidavit_key_run_service_returns_ok_after_successful_initialization() {
        let mut env = new_ocw_env();
        env.ext.execute_with(|| {
            init_fork_graph();
            let routine = InitAffidavitKey { at: 10 }; 
            assert_ok!(routine.can_run());
            let aff_key = get_afdt_key();
            assert!(aff_key.is_none());
            assert_eq!(affidavit_key_count(), 0);
            assert_ok!(routine.run_service());
            assert_err!(routine.can_run(), Error::ActiveAfdtKeyNotYetFinalized);
            run_to_block(30);
            assert_err!(routine.can_run(), Error::AffidavitKeyExists);
            let aff_key = get_finalized_afdt_key();
            assert!(aff_key.is_some());
            assert_eq!(affidavit_key_count(), 1);
        })
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ````````````````````````````````` TRY-ELECTION ````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn try_election_can_run_returns_ok() {
        let mut env = new_ocw_env();
        env.ext.execute_with(|| {
            init_fork_graph();
            let aff_id = generate_affidavit_id();
            insert_active_afdt_key(aff_id.clone()).unwrap();
            let pub_afdt = get_public_key(aff_id.clone()).unwrap();
            run_to_block(300);
            let election_routine = TryElection {
                by: pub_afdt,
                at: 300,
            };
            assert_ok!(election_routine.can_run());
        })
    }

    #[test]
    fn try_election_can_run_returns_err_not_affidavit_period() {
        let mut env = new_ocw_env();
        env.ext.execute_with(|| {
            init_fork_graph();
            let aff_id = generate_affidavit_id();
            insert_active_afdt_key(aff_id.clone()).unwrap();
            let pub_afdt = get_public_key(aff_id.clone()).unwrap();
            run_to_block(250);
            let election_routine = TryElection {
                by: pub_afdt,
                at: 250,
            };
            assert_err!(election_routine.can_run(), Error::NotElectionPeriod);
        })
    }

    #[test]
    fn try_election_run_service_returns_ok_not_election_window() {
        let mut env = new_ocw_env();
        env.ext.execute_with(|| {
            init_fork_graph();
            let aff_id = generate_affidavit_id();
            insert_active_afdt_key(aff_id.clone()).unwrap();
            let pub_afdt = get_public_key(aff_id.clone()).unwrap();
            run_to_block(250);
            let election_routine = TryElection {
                by: pub_afdt,
                at: 250,
            };
            assert_ok!(election_routine.run_service());
        })
    }

    #[test]
    fn try_election_run_service_returns_ok_for_duplicate_election_execution() {
        let mut env = new_ocw_env();
        env.ext.execute_with(|| {
            CurrentSession::put(1);
            SessionStartsAt::put(1);
            init_fork_graph();
            run_to_block(10);
            let aff_id = generate_affidavit_id();
            insert_active_afdt_key(aff_id.clone()).unwrap();
            let pub_afdt = get_public_key(aff_id.clone()).unwrap();

            set_default_user_balance_and_hold(ALICE).unwrap();
            set_default_user_balance_and_hold(ALAN).unwrap();

            enroll_authors_with_default_collateral(vec![ALICE]).unwrap();
            direct_fund_author(ALAN, ALICE, 500).unwrap();

            // Simulate declared affidavit
            let for_session = CurrentSession::get() + 2;
            let afdt_key = get_afdt_key().unwrap();
            AffidavitKeys::insert((for_session, afdt_key), ALICE);
            // Simulate election already executed
            let for_session = CurrentSession::get() + 1;
            ElectsPreparedBy::insert(for_session, (ALICE, 310));

            run_to_block(350);
            let election_routine = TryElection {
                by: pub_afdt,
                at: 350,
            };
            assert_ok!(election_routine.run_service());
            let txs_len = env.pool_state.read().transactions.len();
            assert_eq!(txs_len, 0);
        })
    }

    #[test]
    fn try_election_run_service_returns_ok_when_extrinsic_submitted_succesfully() {
        let mut env = new_ocw_env();
        env.ext.execute_with(|| {
            CurrentSession::put(1);
            SessionStartsAt::put(1);
            init_fork_graph();
            run_to_block(10);
            let aff_id = generate_affidavit_id();
            insert_active_afdt_key(aff_id.clone()).unwrap();
            let pub_afdt = get_public_key(aff_id.clone()).unwrap();

            set_default_user_balance_and_hold(ALICE).unwrap();
            set_default_user_balance_and_hold(ALAN).unwrap();

            enroll_authors_with_default_collateral(vec![ALICE]).unwrap();
            direct_fund_author(ALAN, ALICE, 500).unwrap();

            let aff_window = compute_affidavit_window().unwrap();
            let aff_begin = aff_window.start;
            run_to_block(aff_begin);
            AllowAffidavits::put(true);
            let for_session = CurrentSession::get() + 2;
            let afdt_key = get_finalized_afdt_key().unwrap();
            // declare affidavit simulation
            AffidavitKeys::insert((for_session, afdt_key), ALICE);

            run_to_block(350);
            let election_routine = TryElection {
                by: pub_afdt,
                at: 350,
            };

            let tx_len = env.pool_state.read().transactions.len();
            assert_eq!(tx_len, 0);
            assert_ok!(election_routine.run_service());
            let tx_len = env.pool_state.read().transactions.len();
            assert_eq!(tx_len, 1);
        })
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // `````````````````````````````` DECLARE-AFFIDAVIT ``````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn declare_affidavit_who_returns_public_key() {
        let mut env = new_ocw_env();
        env.ext.execute_with(|| {
            init_fork_graph();
            let init_routine = InitAffidavitKey { at: 10 };
            init_routine.run_service().unwrap();
            run_to_block(30);
            let afdt_key = get_finalized_afdt_key().unwrap();
            let public_key = get_public_key(afdt_key);
            assert!(public_key.is_some());
            let pub_key = public_key.unwrap();

            let who = DeclareAffidavit::who(&30).unwrap();
            assert_eq!(who, pub_key);
        })
    }

    #[test]
    fn declare_affidavit_who_returns_err_expected_to_hold_active_affidavit_key() {
        let mut env = new_ocw_env();
        env.ext.execute_with(|| {
            init_fork_graph();
            let who = DeclareAffidavit::who(&10);
            assert!(who.is_err());
            assert_err!(who, Error::ExpectedToHoldActiveAffidavitKey);
        })
    }

    #[test]
    fn declare_affidavit_who_returns_err_active_afdt_key_not_yet_finalized() {
        let mut env = new_ocw_env();
        env.ext.execute_with(|| {
            init_fork_graph();
            FinalityAfter::put(60_000);
            let init_routine = InitAffidavitKey { at: 10 };
            init_routine.run_service().unwrap();
            run_to_block(15);
            let who = DeclareAffidavit::who(&10);
            assert!(who.is_err());
            assert_err!(who, Error::ActiveAfdtKeyNotYetFinalized);
        })
    }

    #[test]
    fn declare_affidavit_who_returns_err_expected_active_affidavit_key_pair_not_found() {
        let mut env = new_ocw_env();
        env.ext.execute_with(|| {
            init_fork_graph();
            let aff_id: AffidavitId = account("dummy", 1, 1);
            insert_active_afdt_key(aff_id).unwrap();
            run_to_block(20);
            let who = DeclareAffidavit::who(&10);
            assert!(who.is_err());
            assert_err!(who, Error::ExpectedActiveAffidavitKeyPairNotFound);
        })
    }

    #[test]
    fn declare_affidavit_can_run_returns_ok() {
        let mut env = new_ocw_env();
        env.ext.execute_with(|| {
            SessionStartsAt::put(1);
            init_fork_graph();
            let init_routine = InitAffidavitKey { at: 10 };
            init_routine.run_service().unwrap();
            run_to_block(30);
            let afdt_key = get_finalized_afdt_key().unwrap();
            let public_key = get_public_key(afdt_key.clone());
            assert!(public_key.is_some());
            let pub_key = public_key.unwrap();

            set_default_user_balance_and_hold(ALICE).unwrap();
            set_default_user_balance_and_hold(ALAN).unwrap();

            enroll_authors_with_default_collateral(vec![ALICE]).unwrap();
            direct_fund_author(ALAN, ALICE, 500).unwrap();

            let aff_window = compute_affidavit_window().unwrap();
            let aff_begin = aff_window.start;
            run_to_block(aff_begin);
            AllowAffidavits::put(true);
            let declare_routine = DeclareAffidavit {
                by: pub_key,
                at: aff_begin + 5,
            };

            run_to_block(aff_begin + 5);
            assert_err!(declare_routine.can_run(), Error::NextAfdtKeyNotYetFinalized);
            let for_session = CurrentSession::get() + 1;
            // Validate
            AffidavitKeys::insert((for_session, afdt_key), ALICE);
            run_to_block(aff_begin + 30);
            assert_ok!(declare_routine.can_run());
        })
    }

    #[test]
    fn declate_affidavit_run_service_submits_the_extrinsic_and_returns_ok() {
        let mut env = new_ocw_env();
        env.ext.execute_with(|| {
            init_fork_graph();
            SessionStartsAt::put(1);
            let init_routine = InitAffidavitKey { at: 10 };
            init_routine.run_service().unwrap();
            run_to_block(30);
            let afdt_key = get_finalized_afdt_key().unwrap();
            let public_key = get_public_key(afdt_key.clone());
            assert!(public_key.is_some());
            let pub_key = public_key.unwrap();

            set_default_user_balance_and_hold(ALICE).unwrap();
            set_default_user_balance_and_hold(ALAN).unwrap();

            enroll_authors_with_default_collateral(vec![ALICE]).unwrap();
            direct_fund_author(ALAN, ALICE, 500).unwrap();

            let aff_window = compute_affidavit_window().unwrap();
            let aff_begin = aff_window.start;
            run_to_block(aff_begin);
            AllowAffidavits::put(true);
            let declare_routine = DeclareAffidavit {
                by: pub_key,
                at: aff_begin + 5,
            };

            run_to_block(aff_begin + 5);
            assert_err!(declare_routine.can_run(), Error::NextAfdtKeyNotYetFinalized);
            let for_session = CurrentSession::get() + 1;
            // Validate
            AffidavitKeys::insert((for_session, afdt_key), ALICE);
            run_to_block(aff_begin + 30);
            let txs_len = env.pool_state.read().transactions.len();
            assert_eq!(txs_len, 0);
            assert_ok!(declare_routine.run_service());
            let txs_len = env.pool_state.read().transactions.len();
            assert_eq!(txs_len, 1);
        })
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````` ROTATE-AFFIDAVIT-KEY `````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn rotate_affidavit_key_who_returns_public_key() {
        let mut env = new_ocw_env();
        env.ext.execute_with(|| {
            SessionStartsAt::put(1);
            init_fork_graph();
            let init_routine = InitAffidavitKey { at: 10 };
            init_routine.run_service().unwrap();
            run_to_block(30);
            let afdt_key = get_finalized_afdt_key().unwrap();
            let public_key = get_public_key(afdt_key.clone());
            assert!(public_key.is_some());
            let pub_key = public_key.unwrap();

            set_default_user_balance_and_hold(ALICE).unwrap();
            set_default_user_balance_and_hold(ALAN).unwrap();

            enroll_authors_with_default_collateral(vec![ALICE]).unwrap();
            direct_fund_author(ALAN, ALICE, 500).unwrap();

            let aff_window = compute_affidavit_window().unwrap();
            let aff_begin = aff_window.start;
            run_to_block(aff_begin);
            AllowAffidavits::put(true);
            let declare_routine = DeclareAffidavit {
                by: pub_key,
                at: aff_begin + 5,
            };

            run_to_block(aff_begin + 5);
            assert_err!(declare_routine.can_run(), Error::NextAfdtKeyNotYetFinalized);
            let for_session = CurrentSession::get() + 1;
            // Validate
            AffidavitKeys::insert((for_session, afdt_key), ALICE);
            run_to_block(aff_begin + 30);
            let txs_len = env.pool_state.read().transactions.len();
            assert_eq!(txs_len, 0);
            assert_ok!(declare_routine.run_service());
            let txs_len = env.pool_state.read().transactions.len();
            assert_eq!(txs_len, 1);

            let rotate_afdt_key = get_finalized_next_afdt_key();
            assert!(rotate_afdt_key.is_some());
            let rotate_afdt_key = rotate_afdt_key.unwrap();
            let rotate_pub_key = get_public_key(rotate_afdt_key).unwrap();

            let rotate_at = aff_begin + 35;
            let _rotate_routine = RotateAffidavitKey {
                by: rotate_pub_key.clone(),
                at: rotate_at,
            };
            run_to_block(rotate_at);
            let actual_rotate_pub_key = RotateAffidavitKey::who(&rotate_at).unwrap();
            assert_eq!(actual_rotate_pub_key, rotate_pub_key);
        })
    }

    #[test]
    fn rotate_affidavit_key_who_returns_err_expected_to_hold_finalized_next_afdt_key() {
        let mut env = new_ocw_env();
        env.ext.execute_with(|| {
            SessionStartsAt::put(1);
            init_fork_graph();
            let init_routine = InitAffidavitKey { at: 10 };
            init_routine.run_service().unwrap();
            run_to_block(30);
            let afdt_key = get_finalized_afdt_key().unwrap();
            let public_key = get_public_key(afdt_key.clone());
            assert!(public_key.is_some());
            let pub_key = public_key.unwrap();

            set_default_user_balance_and_hold(ALICE).unwrap();
            set_default_user_balance_and_hold(ALAN).unwrap();

            enroll_authors_with_default_collateral(vec![ALICE]).unwrap();
            direct_fund_author(ALAN, ALICE, 500).unwrap();

            let aff_window = compute_affidavit_window().unwrap();
            let aff_begin = aff_window.start;
            run_to_block(aff_begin);
            AllowAffidavits::put(true);
            let declare_routine = DeclareAffidavit {
                by: pub_key,
                at: aff_begin + 5,
            };

            run_to_block(aff_begin + 5);
            assert_err!(declare_routine.can_run(), Error::NextAfdtKeyNotYetFinalized);
            let for_session = CurrentSession::get() + 1;
            // Validate
            AffidavitKeys::insert((for_session, afdt_key), ALICE);

            let rotate_afdt_key = get_next_afdt_key();
            assert!(rotate_afdt_key.is_some());
            let rotate_afdt_key = rotate_afdt_key.unwrap();
            let rotate_pub_key = get_public_key(rotate_afdt_key).unwrap();

            let rotate_at = aff_begin + 10;
            let _rotate_routine = RotateAffidavitKey {
                by: rotate_pub_key.clone(),
                at: rotate_at,
            };
            run_to_block(rotate_at);
            assert_err!(
                RotateAffidavitKey::who(&rotate_at),
                Error::ExpectedToHoldFinalizedNextAffidavitKey
            );
        })
    }

    #[test]
    fn rotate_affidavit_key_can_run_returns_ok() {
        let mut env = new_ocw_env();
        env.ext.execute_with(|| {
            SessionStartsAt::put(1);
            init_fork_graph();
            let init_routine = InitAffidavitKey { at: 10 };
            init_routine.run_service().unwrap();
            run_to_block(30);
            let afdt_key = get_finalized_afdt_key().unwrap();
            let public_key = get_public_key(afdt_key.clone());
            assert!(public_key.is_some());
            let pub_key = public_key.unwrap();

            set_default_user_balance_and_hold(ALICE).unwrap();
            set_default_user_balance_and_hold(ALAN).unwrap();

            enroll_authors_with_default_collateral(vec![ALICE]).unwrap();
            direct_fund_author(ALAN, ALICE, 500).unwrap();

            let aff_window = compute_affidavit_window().unwrap();
            let aff_begin = aff_window.start;
            run_to_block(aff_begin);
            AllowAffidavits::put(true);
            let declare_routine = DeclareAffidavit {
                by: pub_key,
                at: aff_begin + 5,
            };

            run_to_block(aff_begin + 5);
            assert_err!(declare_routine.can_run(), Error::NextAfdtKeyNotYetFinalized);
            let for_session = CurrentSession::get() + 1;
            // Validate
            AffidavitKeys::insert((for_session, afdt_key), ALICE);
            run_to_block(aff_begin + 30);
            let txs_len = env.pool_state.read().transactions.len();
            assert_eq!(txs_len, 0);
            assert_ok!(declare_routine.run_service());
            let txs_len = env.pool_state.read().transactions.len();
            assert_eq!(txs_len, 1);
            let rotate_afdt_key = get_finalized_next_afdt_key();
            assert!(rotate_afdt_key.is_some());
            let rotate_afdt_key = rotate_afdt_key.unwrap();
            let rotate_pub_key = get_public_key(rotate_afdt_key.clone()).unwrap();

            let rotate_at = aff_begin + 35;
            let rotate_routine = RotateAffidavitKey {
                by: rotate_pub_key.clone(),
                at: rotate_at,
            };
            run_to_block(rotate_at);
            let for_session = CurrentSession::get() + 2;
            // Declare affidavit extrinsic executed simulation
            AffidavitKeys::insert((for_session, rotate_afdt_key), ALICE);
            assert_ok!(rotate_routine.can_run());
        })
    }

    #[test]
    fn rotate_affidavit_key_can_run_returns_err_affidavit_tx_awaiting_status() {
        let mut env = new_ocw_env();
        env.ext.execute_with(|| {
            SessionStartsAt::put(1);
            init_fork_graph();
            let init_routine = InitAffidavitKey { at: 10 };
            init_routine.run_service().unwrap();
            run_to_block(30);
            let afdt_key = get_finalized_afdt_key().unwrap();
            let public_key = get_public_key(afdt_key.clone());
            assert!(public_key.is_some());
            let pub_key = public_key.unwrap();

            set_default_user_balance_and_hold(ALICE).unwrap();
            set_default_user_balance_and_hold(ALAN).unwrap();

            enroll_authors_with_default_collateral(vec![ALICE]).unwrap();
            direct_fund_author(ALAN, ALICE, 500).unwrap();

            let aff_window = compute_affidavit_window().unwrap();
            let aff_begin = aff_window.start;
            run_to_block(aff_begin);
            AllowAffidavits::put(true);
            let declare_routine = DeclareAffidavit {
                by: pub_key,
                at: aff_begin + 5,
            };

            run_to_block(aff_begin + 5);
            assert_err!(declare_routine.can_run(), Error::NextAfdtKeyNotYetFinalized);
            let for_session = CurrentSession::get() + 1;
            // Validate
            AffidavitKeys::insert((for_session, afdt_key), ALICE);
            run_to_block(aff_begin + 30);
            let txs_len = env.pool_state.read().transactions.len();
            assert_eq!(txs_len, 0);
            assert_ok!(declare_routine.run_service());
            let txs_len = env.pool_state.read().transactions.len();
            assert_eq!(txs_len, 1);
            let rotate_afdt_key = get_finalized_next_afdt_key();
            assert!(rotate_afdt_key.is_some());
            let rotate_afdt_key = rotate_afdt_key.unwrap();
            let rotate_pub_key = get_public_key(rotate_afdt_key.clone()).unwrap();

            let rotate_at = aff_begin + 35;
            let rotate_routine = RotateAffidavitKey {
                by: rotate_pub_key.clone(),
                at: rotate_at,
            };
            run_to_block(rotate_at);
            // Declare affidavit not yet executed
            assert_err!(rotate_routine.can_run(), Error::AffidavitTxAwaitingStatus);
        })
    }

    #[test]
    fn rotate_affidavit_key_can_run_returns_err_validation_stopped_when_window_expired() {
        let mut env = new_ocw_env();
        env.ext.execute_with(|| {
            SessionStartsAt::put(1);
            init_fork_graph();
            let init_routine = InitAffidavitKey { at: 10 };
            init_routine.run_service().unwrap();
            run_to_block(30);
            let afdt_key = get_finalized_afdt_key().unwrap();
            let public_key = get_public_key(afdt_key.clone());
            assert!(public_key.is_some());
            let pub_key = public_key.unwrap();

            set_default_user_balance_and_hold(ALICE).unwrap();
            set_default_user_balance_and_hold(ALAN).unwrap();

            enroll_authors_with_default_collateral(vec![ALICE]).unwrap();
            direct_fund_author(ALAN, ALICE, 500).unwrap();

            let aff_window = compute_affidavit_window().unwrap();
            let aff_begin = aff_window.start;
            run_to_block(aff_begin);
            AllowAffidavits::put(true);
            let declare_routine = DeclareAffidavit {
                by: pub_key,
                at: aff_begin + 5,
            };

            run_to_block(aff_begin + 5);
            assert_err!(declare_routine.can_run(), Error::NextAfdtKeyNotYetFinalized);
            let for_session = CurrentSession::get() + 1;
            // Validate
            AffidavitKeys::insert((for_session, afdt_key), ALICE);
            run_to_block(aff_begin + 30);
            let txs_len = env.pool_state.read().transactions.len();
            assert_eq!(txs_len, 0);
            assert_ok!(declare_routine.run_service());
            let txs_len = env.pool_state.read().transactions.len();
            assert_eq!(txs_len, 1);
            let rotate_afdt_key = get_finalized_next_afdt_key();
            assert!(rotate_afdt_key.is_some());
            let rotate_afdt_key = rotate_afdt_key.unwrap();
            let rotate_pub_key = get_public_key(rotate_afdt_key.clone()).unwrap();

            let avg_session_len: BlockNumber = NextSessionRotation::average_session_length();
            let rotate_at = avg_session_len + 10;
            let rotate_routine = RotateAffidavitKey {
                by: rotate_pub_key.clone(),
                at: rotate_at,
            };
            run_to_block(rotate_at);
            // Declare affidavit not yet executed and the session window expired
            assert_err!(rotate_routine.can_run(), Error::ValidationStopped);
            let aff_key = get_afdt_key();
            assert!(aff_key.is_none());
            let next_aff_key = get_next_afdt_key();
            assert!(next_aff_key.is_none());
        })
    }

    #[test]
    fn rotate_affidavit_key_run_service_returns_ok() {
        let mut env = new_ocw_env();
        env.ext.execute_with(|| {
            SessionStartsAt::put(1);
            init_fork_graph();
            let init_routine = InitAffidavitKey { at: 10 };
            init_routine.run_service().unwrap();
            run_to_block(30);
            let afdt_key = get_finalized_afdt_key().unwrap();
            let public_key = get_public_key(afdt_key.clone());
            assert!(public_key.is_some());
            let pub_key = public_key.unwrap();

            set_default_user_balance_and_hold(ALICE).unwrap();
            set_default_user_balance_and_hold(ALAN).unwrap();

            enroll_authors_with_default_collateral(vec![ALICE]).unwrap();
            direct_fund_author(ALAN, ALICE, 500).unwrap();

            let aff_window = compute_affidavit_window().unwrap();
            let aff_begin = aff_window.start;
            run_to_block(aff_begin);
            AllowAffidavits::put(true);
            let declare_routine = DeclareAffidavit {
                by: pub_key,
                at: aff_begin + 5,
            };

            run_to_block(aff_begin + 5);
            assert_err!(declare_routine.can_run(), Error::NextAfdtKeyNotYetFinalized);
            let for_session = CurrentSession::get() + 1;
            // Validate
            AffidavitKeys::insert((for_session, afdt_key), ALICE);
            run_to_block(aff_begin + 30);
            let txs_len = env.pool_state.read().transactions.len();
            assert_eq!(txs_len, 0);
            assert_ok!(declare_routine.run_service());
            let txs_len = env.pool_state.read().transactions.len();
            assert_eq!(txs_len, 1);
            let rotate_afdt_key = get_finalized_next_afdt_key();
            assert!(rotate_afdt_key.is_some());
            let rotate_afdt_key = rotate_afdt_key.unwrap();
            let rotate_pub_key = get_public_key(rotate_afdt_key.clone()).unwrap();

            let rotate_at = aff_begin + 35;
            let rotate_routine = RotateAffidavitKey {
                by: rotate_pub_key.clone(),
                at: rotate_at,
            };
            run_to_block(rotate_at);
            let for_session = CurrentSession::get() + 2;
            // Declare affidavit extrinsic executed simulation
            AffidavitKeys::insert((for_session, rotate_afdt_key.clone()), ALICE);

            assert_ok!(rotate_routine.run_service());
            let next_afdt_key = get_next_afdt_key();
            assert!(next_afdt_key.is_none());
            let afdt_key = get_afdt_key();
            assert!(afdt_key.is_some());
            assert_eq!(afdt_key.unwrap(), rotate_afdt_key);
        })
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // `````````````````````````` FINALIZED (OFFCHAIN-STORE) `````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    #[test]
    fn finalized_insert_success() {
        let mut env = new_ocw_env();
        env.ext.execute_with(|| {
            init_fork_graph();
            let key = b"ACTIVE_KEY";
            let value: AccountId = account("dummyid", 1, 1);
            assert_ok!(FinalizedInitAfdtKey::insert(key, &value, None, None));

            let fork_aware_value = ForkAwareInitAfdtKey::get(key, None, None).unwrap();
            assert!(fork_aware_value.is_some());
            let value_hash = fork_aware_value.unwrap();

            let persistent_value = PersistentInitAfdtKey::get(key, None, None).unwrap();
            assert!(persistent_value.is_some());
            let ledger = persistent_value.unwrap();
            let observation = ledger.0.get(&value_hash).unwrap();
            assert_eq!(observation.first_seen, 12000);
            assert_eq!(observation.last_seen, 12000);
            assert_eq!(observation.blocks_seen, 0);
            assert_eq!(observation.value, value);
        })
    }

    #[test]
    fn finalized_get_success() {
        let mut env = new_ocw_env();
        env.ext.execute_with(|| {
            let key = b"ACTIVE_KEY";
            init_fork_graph();
            let value: AccountId = account("dummyid", 1, 1);
            assert_ok!(FinalizedInitAfdtKey::insert(key, &value, None, None));

            // since, the first_seen + finalty_after > last_seen and obs_block < finalty_ticks
            // confidance = Unsafe(value)
            let finalized_get = FinalizedInitAfdtKey::get(key, None, None).unwrap();
            assert!(finalized_get.is_some());
            let confidance_value = finalized_get.unwrap();
            assert_eq!(confidance_value, Confidence::Unsafe(value.clone()));

            // since, the first_seen + finalty_after < last_seen and obs_block < finalty_ticks
            // confidance = Risky(value)
            run_to_block_with_finalized_key(12, key);
            let finalized_get = FinalizedInitAfdtKey::get(key, None, None).unwrap();
            assert!(finalized_get.is_some());
            let confidance_value = finalized_get.unwrap();
            assert_eq!(confidance_value, Confidence::Risky(value.clone()));

            // since, the first_seen + finalty_after < last_seen and obs_block >= finalty_ticks
            // confidance = Safe(value)
            run_to_block_with_finalized_key(17, key);
            let finalized_get = FinalizedInitAfdtKey::get(key, None, None).unwrap();
            assert!(finalized_get.is_some());
            let confidance_value = finalized_get.unwrap();
            assert_eq!(confidance_value, Confidence::Safe(value));
        })
    }

    #[test]
    fn finalized_get_returns_err_due_to_hanging_value() {
        let mut env = new_ocw_env();
        env.ext.execute_with(|| {
            init_fork_graph();
            let key = b"ACTIVE_KEY";
            let value: AccountId = account("dummyid", 1, 1);
            let hash = blake2_256(&value.encode());
            let value_hash = ValueHash::new(hash);
            assert_ok!(ForkAwareInitAfdtKey::insert(key, &value_hash, None, None));

            let finalized_get = FinalizedInitAfdtKey::get(key, None, None);
            assert!(finalized_get.is_err());
            let e = finalized_get.unwrap_err();
            assert_eq!(e, Error::ActiveAfdtKeyFinalizedHangingValue.into());

            // forkaware entry cleaned due to hanging value
            let forkaware_lookup = ForkAwareInitAfdtKey::get(key, None, None).unwrap();
            assert!(forkaware_lookup.is_none());
        })
    }

    #[test]
    fn finalized_get_returns_err_due_to_hanging_hash() {
        let mut env = new_ocw_env();
        env.ext.execute_with(|| {
            init_fork_graph();
            let key = b"ACTIVE_KEY";
            let value: AccountId = account("dummyid", 1, 1);
            let hash = blake2_256(&value.encode());
            let value_hash = ValueHash::new(hash);

            assert_ok!(ForkAwareInitAfdtKey::insert(key, &value_hash, None, None));

            let other_value: AccountId = account("otherdummyid", 1, 1);
            let other_hash = blake2_256(&other_value.encode());
            let other_value_hash = ValueHash::new(other_hash);

            let observation = Observation::<Test, AccountId> {
                first_seen: 6000,
                last_seen: 6000,
                blocks_seen: 0,
                value: other_value,
            };

            let mut map = BTreeMap::new();
            map.insert(other_value_hash, observation);

            let ledger = Ledger::<Test, AccountId>(map);

            assert_ok!(PersistentInitAfdtKey::insert(
                key,
                &ledger,
                None,
                None
            ));

            let finalized_get = FinalizedInitAfdtKey::get(key, None, None);
            assert!(finalized_get.is_err());

            let e = finalized_get.unwrap_err();
            assert_eq!(e, Error::ActiveAfdtKeySpeculativeHangingHash.into());

            let forkaware_lookup = ForkAwareInitAfdtKey::get(key, None, None).unwrap();
            assert!(forkaware_lookup.is_none());
            let persistent = PersistentInitAfdtKey::get(key, None, None).unwrap();
            assert!(persistent.is_some());
        })
    }

    #[test]
    fn finalized_remove_success() {
        let mut env = new_ocw_env();
        env.ext.execute_with(|| {
            init_fork_graph();
            let key = b"ACTIVE_KEY";
            let value: AccountId = account("dummyid", 1, 1);
            assert_ok!(FinalizedInitAfdtKey::insert(key, &value, None, None));

            let finalized_inspect = FinalizedInitAfdtKey::get(key, None, None).unwrap();
            assert!(finalized_inspect.is_some());

            assert_ok!(FinalizedInitAfdtKey::remove(key, None, None));

            let finalized_inspect = FinalizedInitAfdtKey::get(key, None, None).unwrap();
            assert!(finalized_inspect.is_none());
        })
    }

    #[test]
    fn finalized_mutate_success() {
        let mut env = new_ocw_env();
        env.ext.execute_with(|| {
            init_fork_graph();
            let key = b"ACTIVE_KEY";
            let value: AccountId = account("dummyid", 1, 1);
            assert_ok!(FinalizedInitAfdtKey::insert(key, &value, None, None));

            let new_val: AccountId = account("newdummyid", 1, 1);
            assert_ok!(FinalizedInitAfdtKey::mutate(
                key,
                |val| {
                    match val {
                        Ok(Some(_v)) => Ok(new_val.clone()),
                        Ok(None) => Ok(new_val.clone()),
                        Err(e) => Err(e),
                    }
                },
                None,
                None
            ));

            let finalized_get = FinalizedInitAfdtKey::get(key, None, None).unwrap();
            let new_finalized_value = match finalized_get {
                Some(new_val) => match new_val {
                    Confidence::Unsafe(id) => id,
                    _ => value,
                },
                None => value,
            };

            assert_eq!(new_finalized_value, new_val);
            let persistent = PersistentInitAfdtKey::get(key, None, None).unwrap().unwrap();
            let (_, obs) = persistent.0.iter().next().unwrap();
            assert_eq!(obs.blocks_seen, 0);
            assert_eq!(obs.value, new_val);
        })
    }
}

// ===============================================================================
// ````````````````````````````````` SERIAL TESTS ````````````````````````````````
// ===============================================================================

// These tests are isolated in a dedicated module because they rely on a **global logger state**.
// Rust tests run in parallel by default, which causes race conditions and flaky failures when
// multiple tests attempt to read/write from the same global logger.
//
// To avoid this, each test in this module is:
//   - Annotated with `#[serial]` (from the `serial_test` crate) to enforce sequential execution
//   - Marked with `#[ignore]` to prevent accidental execution during standard test runs
//
// Running these tests:
// --------------------
// Since they are ignored by default, you must run them explicitly and ensure single-threaded
// execution:
//
// `cargo test serial_tests -- --ignored --test-threads=1`
//
#[cfg(test)]
mod serial_tests {
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` IMPORTS ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // --- Local crate imports ---
    use crate::mock::*;

    // --- Test Utils ---
    use serial_test::serial;

    // --- FRAME Suite ---
    use frame_suite::routines::*;

    // --- FRAME Support ---
    use frame_support::assert_ok;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` LOGGING ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[serial]
    #[test]
    #[ignore = "relies on global logger; fails under parallel test execution"]
    fn init_affidavit_key_on_ran_service() {
        chain_manager_test_ext().execute_with(|| {
            let logger = init_logger();

            System::set_block_number(105);
            let init_afdt_routine = InitAffidavitKey { at: 105 };
            InitAffidavitKey::on_ran_service(&init_afdt_routine);

            let record = logger.last().unwrap();
            assert_eq!(record.target(), "AFFIDAVIT");
            assert_eq!(record.level(), log::Level::Debug);
            let expected_msg = format!(
                "🧱 [105] 🐛 [Debug] 🎯 [AFFIDAVIT] 🧾 Module(9): InitAffidavitKeyRoutineSuccess"
            );
            let actual_msg = record.args().to_string();
            assert_eq!(actual_msg, expected_msg);
        })
    }

    #[serial]
    #[test]
    #[ignore = "relies on global logger; fails under parallel test execution"]
    fn try_election_on_ran_service() {
        let mut env = new_ocw_env();
        env.ext.execute_with(|| {
            let logger = init_logger();

            System::set_block_number(105);
            let by = generate_affidavit_keypair();
            let try_election_routine = TryElection { by: by, at: 105 };
            TryElection::on_ran_service(&try_election_routine);

            let record = logger.last().unwrap();
            assert_eq!(record.target(), "ELECTION");
            assert_eq!(record.level(), log::Level::Debug);
            let expected_msg =
                format!("🧱 [105] 🐛 [Debug] 🎯 [ELECTION] 🧾 Module(9): TryElectionRoutineSuccess");
            let actual_msg = record.args().to_string();
            assert_eq!(actual_msg, expected_msg);
        })
    }

    #[serial]
    #[test]
    #[ignore = "relies on global logger; fails under parallel test execution"]
    fn declare_affidavit_on_ran_service() {
        let mut env = new_ocw_env();
        env.ext.execute_with(|| {
            let logger = init_logger();

            System::set_block_number(105);
            let by = generate_affidavit_keypair();
            let declare_afdt_routine = DeclareAffidavit { by, at: 105 };
            DeclareAffidavit::on_ran_service(&declare_afdt_routine);

            let record = logger.last().unwrap();
            assert_eq!(record.target(), "AFFIDAVIT");
            assert_eq!(record.level(), log::Level::Debug);
            let expected_msg = format!(
                "🧱 [105] 🐛 [Debug] 🎯 [AFFIDAVIT] 🧾 Module(9): DeclarAffidavitRoutineSuccess"
            );
            let actual_msg = record.args().to_string();
            assert_eq!(actual_msg, expected_msg);
        })
    }

    #[serial]
    #[test]
    #[ignore = "relies on global logger; fails under parallel test execution"]
    fn rotate_affidavit_key_on_ran_service() {
        let mut env = new_ocw_env();
        env.ext.execute_with(|| {
            let logger = init_logger();

            System::set_block_number(105);
            let by = generate_affidavit_keypair();
            let rotate_afdt_routine = RotateAffidavitKey { by, at: 105 };
            RotateAffidavitKey::on_ran_service(&rotate_afdt_routine);

            let record = logger.last().unwrap();
            assert_eq!(record.target(), "AFFIDAVIT");
            assert_eq!(record.level(), log::Level::Debug);
            let expected_msg = format!(
                "🧱 [105] 🐛 [Debug] 🎯 [AFFIDAVIT] 🧾 Module(9): RotateAffidavitKeyRoutineSuccess"
            );
            let actual_msg = record.args().to_string();
            assert_eq!(actual_msg, expected_msg);
        })
    }

    #[serial]
    #[test]
    #[ignore = "relies on global logger; fails under parallel test execution"]
    fn try_election_run_service_returns_ok_when_active_afdt_key_is_not_declared_yet() {
        let mut env = new_ocw_env();
        env.ext.execute_with(|| {
            init_fork_graph();
            let log_record = init_logger();
            let aff_id = generate_affidavit_id();
            insert_active_afdt_key(aff_id.clone()).unwrap();
            let pub_afdt = get_public_key(aff_id.clone()).unwrap();
            run_to_block(300);
            let election_routine = TryElection {
                by: pub_afdt,
                at: 300,
            };
            assert_ok!(election_routine.run_service());

            let last_log = log_record.last().unwrap();
            assert_eq!(last_log.level(), log::Level::Debug);
            assert!(last_log
                .args()
                .to_string()
                .contains("AffidavitKeyForDeclaration"));
            assert_eq!(last_log.target(), "ELECTION");
        })
    }
}