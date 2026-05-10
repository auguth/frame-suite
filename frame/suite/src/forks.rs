// SPDX-License-Identifier: MPL-1.0
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
// ```````````````````````````` FORK-LOCAL UTILITIES `````````````````````````````
// ===============================================================================

//! Fork-local utilities for pallet offchain workers (OCWs).
//!
//! This module does not provide canonical chain selection or consensus.
//!
//! Instead, it runs on top of a pallet's OCW execution and maintains a
//! deterministic local fork graph so the pallet can answer:
//!
//! ```ignore
//! "what was my local state on this branch?"
//! ```
//!
//! A [`Branch`] represents a continuous stream of blocks extending on top
//! of each other along the same path.
//!
//! ```text
//! A -> B -> C -> D
//! same branch
//! ```
//!
//! As long as blocks continue as direct children, the same branch is reused
//! and only the branch head moves forward.
//!
//! When a division occurs (a sibling block appears at the same ancestry),
//! a new fork branch is created.
//!
//! ```text
//! A -> B -> C
//!         |-- D
//!         |-- D'
//! ```
//!
//! Here:
//!
//! - `D` is the original branch continuation
//! - `D'` is the new sibling branch
//! - `A -> B -> C` is the parent branch of both paths
//!
//! Each branch carries its own fork-local scope through [`ForkScopes`].
//!
//! When a new sibling branch is created, it inherits from the parent branch
//! using [`Accrete`]:
//!
//! ```text
//! scope(D') = scope(C).accrete()
//! ```
//!
//! This allows each fork path to maintain isolated local state while still
//! preserving inherited lineage state.
//!
//! ## Usage
//!
//! Every pallet using this system should begin its OCW execution with
//! [`ForksHandler::start`]:
//!
//! ```ignore
//! fn offchain_worker(block_number: BlockNumberFor<T>) {
//!     <Self as ForksHandler<T, MyForkScope>>::start(
//!         Some("my-pallet"),
//!         Some(LogFormatter::default()),
//!         || {
//!             // pallet-specific OCW logic here
//!         }
//!     );
//! }
//! ```
//!
//! [`ForksHandler::start`] handles:
//!
//! - longest-chain extension vs sibling fork creation
//! - missing branch recovery (during client inactivity)
//! - safe branch resolution before OCW logic executes
//!
//! The OCW closure (main logic) runs only after branch state is
//! valid and ready to map the fork graph.
//!
//! ## Navigation
//!
//! Since fork resolution is delayed by one block:
//!
//! ```ignore
//! block N executes
//! -> block N - 1 is resolved and persisted
//! ```
//!
//! the current executing block is not yet inserted into the local fork graph.
//!
//! For normal OCW access, use [`ForksHandler::get_prev_block_branch`]
//! to retrieve the previous block's (`N - 1`) resolved branch.
//!
//! From that branch, navigation can continue using [`ForkAction`] and
//! [`ForksHandler::transition`], or helpers like:
//!
//! - [`ForksHandler::get_block_branch`]
//! - [`ForksHandler::get_prev_branch`]
//! - [`ForksHandler::get_branch`]
//!
//! This allows movement across:
//!
//! - parent branches
//! - child branches
//! - sibling branches
//! - root ancestry
//!
//! This provides deterministic branch-local traversal without relying on
//! canonical consensus routing.
//!
//! The system is intentionally fork-aware, scope-first, and best-effort:
//! it tracks local execution branches, not global consensus finality.

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- Local-Crate ---
use crate::{Accrete, LogFormatter, Logging, Portable};

// --- Scale-codec ---
use codec::{Decode, Encode};

// --- Rust-core (no-std) ---
use core::fmt::Debug;

// --- FRAME System ---
use frame_system::{pallet_prelude::BlockNumberFor, Config, Pallet};

// --- Substrate Primitives ---
use sp_core::blake2_256;
use sp_std::vec;
use sp_runtime::{
    offchain::storage::{MutateStorageError, StorageValueRef},
    traits::{One, Saturating, Zero},
    DispatchError, Vec
};

// ===============================================================================
// `````````````````````````````````` CONSTANTS ``````````````````````````````````
// ===============================================================================

/// Highest known longest-chain head used for fork detection.
///
/// If a new block extends past `HEAD_BLOCK`,
/// it is treated as longest-chain extension.
///
/// If another block appears at the same height or lower,
/// it is treated as a sibling fork.
///
/// Sibling fork detection is best-effort:
///
/// a lower block may still be the head of its own valid fork,
/// but it is treated as a sibling branch of the nearest known path.
///
/// This intentionally favors fewer branch creations
/// and reduced storage growth over perfect historical fork reconstruction.
pub const HEAD_BLOCK: &'static [u8] = b"LOCAL_HEAD_BLOCK";

// ===============================================================================
// `````````````````````````````````` STRUCTURES `````````````````````````````````
// ===============================================================================

/// Fork branch details pertaining to a block and the
/// specialized scope state defined by the pallet/module
/// for which the local fork graph exists.
///
/// Each branch represents the traversed path generation
/// from its local root.
///
/// This root is not necessarily true genesis, but in the
/// best-case scenario it is the original genesis ancestry.
///
/// If the next block is a direct child on top of the same path,
/// the same branch structure is shared and only the branch head
/// moves forward.
///
/// ```text
/// Direct child progression:
///
/// A -> B -> C -> D
/// same branch
///
/// only:
/// head = D
/// ```
///
/// If the next block becomes a sibling block, it constitutes
/// a fork and a new branch is created.
///
/// That new branch takes an [`Accrete`] over the previous
/// scope state so all further state becomes localized
/// to that fork path.
///
/// ```text
/// Sibling fork:
///
/// A -> B -> C
///         |-- D
///         |-- D'
///
/// D  = original branch
/// D' = new sibling branch
///
/// scope(D') = scope(D).accrete()
/// ```
///
/// This structure also stores enough ancestry details to
/// manually traverse:
///
/// - parent branches
/// - sibling branches
/// - nested forks
///
/// allowing full local fork graph inspection.
#[derive(Clone, Debug, Encode, Decode)]
pub struct Branch<T: Config, S: ForkScopes> {
    /// Structural parent branch.
    ///
    /// `None` for synthetic recovery root branches
    /// or initial local roots.
    pub parent: Option<[u8; 32]>,

    /// Latest block height currently owned
    /// by this branch.
    pub head: BlockNumberFor<T>,

    /// Pallet-local fork scope carried by
    /// this branch lineage.
    pub scope: S,

    /// Stable ancestry root for deterministic
    /// branch identity.
    ///
    /// Usually this is the parent block hash of the
    /// block where this fork graph started.
    ///
    /// Example:
    ///
    /// - genesis block start: parent hash = [0; 32]
    /// - mid-chain fork start: parent hash of that fork root
    ///
    /// Shared across all sibling forks created
    /// from that same branch origin.
    pub genesis: [u8; 32],

    /// Fork lineage path from genesis.
    ///
    /// ```text
    /// Root:
    /// A -> B -> C
    /// counter = []
    ///
    /// First sibling fork:
    /// A -> B -> C
    ///         |-- C' [0]
    ///
    /// Second sibling fork:
    /// A -> B -> C
    ///         |-- C'  [0]
    ///         |-- C'' [1]
    ///
    /// Nested fork:
    /// A -> B -> C
    ///         |-- C'  [0]
    ///              |-- D' [0, 0]
    ///         |-- C'' [1]
    ///              |-- D' [1, 0]
    ///              |-- D'' [1, 1]
    /// ```
    pub counter: Vec<u32>,
}

// ===============================================================================
// ```````````````````````````````````` ENUMS ````````````````````````````````````
// ===============================================================================

/// Deterministic branch traversal and navigation actions
/// for moving across the local fork graph.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ForkAction {

    /// Move upward N parent branches.
    ///
    /// Before:
    /// A -> B -> C
    ///         |-- C'
    ///             |-- D'
    ///                 ^
    ///
    /// Example:
    /// MoveToParentBranchBack(2)
    ///
    /// After:
    /// A -> B -> C
    ///           ^
    MoveToParentBranchBack(u32),

    /// Move to the direct parent branch.
    ///
    /// Before:
    /// A -> B -> C
    ///         |-- C'
    ///             ^
    ///
    /// After:
    /// A -> B -> C
    ///           ^
    MoveToParentBranch,

    /// Move forward into the N-th child branch if it exists.
    ///
    /// Before:
    /// A -> B -> C
    ///         ^
    ///         |-- C'
    ///         |-- C''
    ///
    /// Example:
    /// MoveToChildBranch(1)
    ///
    /// After:
    /// A -> B -> C
    ///         |-- C''
    ///             ^
    MoveToChildBranch(u32),

    /// Move forward into the first child branch if it exists.
    ///
    /// Before:
    /// A -> B -> C
    ///           ^
    ///         |-- C'
    ///
    /// After:
    /// A -> B -> C
    ///         |-- C'
    ///             ^
    ///
    /// Deterministic traversal:
    /// always child index 0.
    MoveToNextChildBranch,

    /// Move to a specific sibling branch index.
    ///
    /// If unavailable, remain on current branch.
    ///
    /// Before:
    /// A -> B -> C
    ///         |-- D
    ///         |   ^
    ///         |-- D'
    ///
    /// After:
    /// A -> B -> C
    ///         |-- D
    ///         |-- D'
    ///             ^
    MoveToSiblingBranch(u32),

    /// Move to the next sibling branch if it exists.
    ///
    /// Before:
    /// A -> B -> C
    ///         |-- D
    ///         |   ^
    ///         |-- D'
    ///
    /// After:
    /// A -> B -> C
    ///         |-- D
    ///         |-- D'
    ///             ^
    MoveToNextSiblingBranch,

    /// Move to the previous sibling branch if it exists.
    ///
    /// Before:
    /// A -> B -> C
    ///         |-- D
    ///         |-- D'
    ///             ^
    ///
    /// After:
    /// A -> B -> C
    ///         |-- D
    ///         |   ^
    ///         |-- D'
    MoveToPreviousSiblingBranch,

    /// Jump to the oldest reachable ancestry root.
    ///
    /// This may be true genesis ancestry or a synthetic
    /// recovery root depending on recovery history.
    ///
    /// Before:
    /// A -> B -> C
    ///         |-- C' -> D'
    ///                  ^
    ///
    /// After:
    /// A
    /// ^
    MoveToRootBranch,

}

// ===============================================================================
// ```````````````````````````````````` TRAITS ```````````````````````````````````
// ===============================================================================

/// A scope is an abstract area for storing branch-local state and anything
/// logically related to that fork, such as:
///
/// - values
/// - references
/// - pointers
/// - indexes
/// - cached storage views
/// - execution context
/// - fork-local metadata
/// - lineage-dependent runtime state
///
/// `ForkScopes` is generational:
///
/// - [`Default`] provides the empty first generation
/// - [`Portable`] provides codec-safe storage and cloning
/// - [`Accrete`] allows each new branch to inherit and extend scope
///
/// When a new fork branch is created, it accretes from its parent branch,
/// carrying forward previous generations while starting a fresh local layer.
///
/// This makes the newest generation the full reachable state, avoiding
/// repeated traversal of older branch ancestry during inspection.
pub trait ForkScopes: Portable + Default + Debug + Accrete {}

/// Blanked Implementation for all types that satisfy the trait impls.
impl<T> ForkScopes for T where T: Portable + Default + Debug + Accrete {}

/// Fork-aware local branch collection framework for pallet/module state.
///
/// This trait allows a pallet (or module using OCWs) to maintain its own
/// deterministic local fork graph independent of chain consensus.
///
/// It is tightly coupled to FRAME System and uses:
///
/// - current block number
/// - parent hash
/// - historical block hashes
///
/// to resolve and recover local branch state.
///
/// Each implementation defines:
///
/// - its own `TAG` (storage namespace)
/// - its own `ForkScope` (local state tracked per branch)
///
/// This lets every pallet answer:
///
/// ```ignore
/// "what was my local state on this branch?"
/// ```
///
/// Useful for:
///
/// - OCW-derived state
/// - local indexing and aggregation
/// - branch-aware replay
/// - deterministic recovery after storage loss
///
/// ## OCW execution model
///
/// Offchain workers should always begin with:
///
/// ```ignore
/// fn offchain_worker(block_number: BlockNumberFor<T>) {
///     <Self as ForksHandler<T, MyForkScope>>::start(
///         Some("my-pallet"),
///         Some(LogFormatter::default()),
///         || {
///             // pallet-specific OCW logic here
///         }
///     );
/// }
/// ```
///
/// [`Self::start`] is the only valid entry point.
///
/// It handles:
///
/// - longest-chain extension vs sibling fork creation
/// - recovery of missing or corrupted branch state
/// - conditions where OCW execution should be skipped
///
/// The OCW closure executes only after branch resolution
/// and recovery have completed.
///
/// ## Scope safety
///
/// Fork scope state is recoverable, but not permanently trusted.
///
/// Recovery may recreate synthetic forks and restore only the
/// minimum valid state required for continued execution,
/// not exact historical lineage.
///
/// Fork recovery guarantees execution continuity,
/// not perfect reconstruction.
///
/// ## Query model
///
/// Branch resolution is intentionally delayed by one block.
///
/// The current executing block is not inserted into the fork graph
/// during its own OCW execution.
///
/// Instead:
///
/// ```ignore
/// block N OCW executes
/// -> block N - 1 is resolved and persisted
/// ```
///
/// because the current block hash is not yet safely available
/// for deterministic fork routing.
///
/// This means queries during OCW execution resolve the
/// previous block's branch by default.
///
/// Additional traversal across parent, sibling,
/// and canonical ancestry is available through
/// branch access helpers.
pub trait ForksHandler<T: Config, S: ForkScopes>:
    Logging<BlockNumberFor<T>, Logger = DispatchError> + Sized
{
    /// Storage namespace prefix for all fork-local keys.
    ///
    /// Used to isolate multiple fork-graphs with special scopes.
    const TAG: &[u8];

    /// Maximum sibling forks allowed from a single branch point.
    ///
    /// Once exceeded, no additional sibling branch is created
    /// and [`Self::max_forks`] is triggered.
    const MAX_FORKS: u32;

    /// Maximum reverse traversal attempts during recovery.
    ///
    /// Used when branch resolution fails and the system
    /// walks backward to find the nearest recoverable branch state.
    ///
    /// Prevents unbounded historical scanning.
    const MAX_RECOVER_TRAVERSAL: u32;

    /// Start fork resolution for the previous block and execute OCW logic
    /// inside the resolved branch environment.
    ///
    /// This is the only valid OCW entry point.
    ///
    /// It determines:
    ///
    /// - longest-chain extension vs sibling fork creation
    /// - divider / branch recovery when storage is missing
    /// - corruption handling for decode failures
    /// - mutation conflict promotion into sibling forks
    /// - conditions where OCW execution should be skipped
    ///
    /// Resolution is intentionally delayed by one block:
    ///
    /// ```ignore
    /// block N OCW executes
    /// -> block N - 1 is resolved and persisted
    /// ```
    ///
    /// because the current block hash is not yet safely available
    /// for deterministic fork routing.
    ///
    /// The provided `ocw` closure runs only after:
    ///
    /// - branch resolution is complete
    /// - recovery (if needed) has finished
    /// - block -> divider -> branch invariants are restored
    ///
    /// This guarantees OCW logic executes only inside a valid
    /// fork-aware branch context.
    fn start<F: FnOnce()>(
        target: Option<&str>,
        fmt: Option<LogFormatter<BlockNumberFor<T>, Self::Level>>,
        ocw: F,
    ) { 
        // Current executing OCW block (block N).
        //
        // Fork registration is intentionally delayed by one block,
        // so this execution resolves and persists: block N - 1
        //
        // instead of the currently executing block itself.
        let block = Pallet::<T>::block_number();

        // Early bootstrap boundary detection.
        //
        // At the earliest chain boundary: parent == grand_parent
        //
        // because historical hashes are not yet available and
        // saturating subtraction collapses both values.
        //
        // In this state there is not enough ancestry to safely
        // resolve fork lineage, so execution is skipped.
        let actual_parent_block = block.saturating_sub(One::one());
        let actual_grandparent_block = block.saturating_sub(2u8.into());

        if actual_parent_block == actual_grandparent_block {
            return;
        }

        // Shift execution target: block N -> resolve block N - 1
        //
        // This gives access to:
        //
        // - exact current hash of N - 1
        // - exact parent hash of N - 2
        //
        // which removes sibling ambiguity and allows deterministic
        // branch routing using fully materialized hashes.
        let block = block.saturating_sub(One::one());

        // Recovery loop.
        //
        // Recovery handlers return: Ok(()) => continue
        // so execution restarts cleanly after repairing storage.
        //
        // In practice this usually runs:
        // - once for normal execution
        // - twice when recovery is required
        let _ = loop {
            // Exact block being inserted into the fork graph.
            //
            // Named `current` for branch semantics,
            // but structurally this is: block N - 1
            let current = Pallet::<T>::block_hash(block);

            // Parent of the resolved block.
            //
            // parent(current) == block N - 2
            let parent = Pallet::<T>::block_hash(block.saturating_sub(One::one()));

            // Highest known longest-chain boundary.
            //
            // Used only for fork detection:
            //
            //     block <= head -> sibling fork path
            //     block >  head -> longest-chain extension
            //
            // This is best-effort and intentionally favors:
            //
            // - fewer new branches
            // - lower storage growth
            //
            // over perfect historical fork reconstruction.
            //
            // (branch a)  (branch a)  (branch a)
            // A ----------B-----------C
            //
            // is same as
            //                      A (branch a)
            //                       \
            //                        B (branch a.a)
            //                         \
            //                          C (branch a.b)
            let head = match Self::get_head() {
                Some(v) => v,
                None => {
                    let initial_head = block.saturating_sub(One::one());
                    store_encoded([Self::TAG, HEAD_BLOCK].concat(), &initial_head);
                    initial_head
                }
            };

            // Divider lookup: parent -> divider
            //
            // Divider is the routing layer between blocks and branches.
            //
            // Multiple sequential blocks of the same ancestry:
            //
            //     A -> B -> C -> D
            //
            // share the same branch structure and local scope lineage,
            // so they resolve into the same branch and only extend its head.
            //
            // OCW scope mutation happens only after branch resolution
            // and graph persistence complete.
            //
            // Under normal execution this behaves like sequential runtime:
            //
            // blocks of the same ancestry only build forward on top of
            // the existing branch, so mutation conflicts are not expected.
            //
            // A conflict may still happen if a previous OCW is still running
            // while the next block begins execution asynchronously.
            //
            // In that case mutation is resolved by promoting the later writer
            // into a sibling branch (clone on mutation conflict),
            // because execution cannot safely wait or abort without first
            // preserving deterministic fork graph progression.
            let divider_hash = Self::get_divider(parent);

            // Missing divider means ancestry routing is broken.
            //
            // Recovery restores only the minimum valid scope
            // required for execution continuity, not exact
            // historical fork reconstruction.
            if divider_hash.is_none() {
                match Self::parent_divider_unavailable(block, target, fmt) {
                    Ok(_) => continue,
                    Err(e) => break e,
                }
            }   
            
            // Full branch resolution: parent -> dividerkey -> branchkey
            //
            // Parent's (N-2) Branch Key
            let branch_hash = Self::get_branch_hash(parent);

            // SIBLING FORK PATH
            //
            // Another block already occupied this height: block <= head
            // so this is treated as a sibling fork.
            if block <= head {
                let prev_branch = match branch_hash {
                    Some(h) => match Self::get_branch(&h) {
                        Some(b) => b,
                        
                        // Divider exists but branch payload is missing.
                        None => match Self::parent_branch_unavailable(block, target, fmt) {
                            Ok(_) => continue,
                            Err(e) => break e,
                        },
                    },
                    
                    // Divider exists but deeper branch resolution failed.
                    None => match Self::parent_branch_hash_unavailable(block, target, fmt) {
                        Ok(_) => continue,
                        Err(e) => break e,
                    },
                };

                // New sibling receives:
                //
                // - logically accreted scope
                // - same ancestry root (genesis) continued
                // - same lineage path until sibling index append
                let scope = prev_branch.scope.accrete();
                let genesis = prev_branch.genesis;
                let mut counter = prev_branch.counter.clone();

                // Find next available sibling slot:
                //
                // [x.0], [x.1], [x.2], ...
                let mut i = 0u32;
                let k = loop {
                    let mut try_counter = counter.clone();
                    try_counter.push(i);

                    let try_branch_hash = branch_key(Self::TAG, genesis, &try_counter);

                    // First unused deterministic sibling slot found.
                    if load_value::<Branch<T, S>>(&try_branch_hash).is_none() {
                        break Some(i);
                    }

                    i += 1;

                    if i > Self::MAX_FORKS {
                        break None;
                    }
                };

                let Some(new_counter) = k else {
                    match Self::max_forks(block, target, fmt) {
                        Ok(_) => continue,
                        Err(e) => break e,
                    }
                };
                
                // Append the sibling fork as a new counter.
                counter.push(new_counter);

                // Permanent deterministic branch identity: genesis + updated lineage counter
                let new_branch_hash = branch_key(Self::TAG, genesis, &counter);

                let new_branch = Branch::<T, S> {
                    parent: branch_hash,
                    head: block,
                    scope,
                    genesis,
                    counter,
                };
                store_encoded(&new_branch_hash, &new_branch);

                // Divider identity: parent + branchkey
                //
                // allows multiple sibling branches from the same parent without overwrite.
                let new_divider_hash = divider_key(Self::TAG, parent, new_branch_hash);
                store_encoded(&new_divider_hash, &new_branch_hash);

                // Final block key-derivation: block -> divider -> branch for N-1 block
                let block_hash = block_key(Self::TAG, current);
                store_encoded(&block_hash, &new_divider_hash);

                // Run OCW logic and return
                ocw();
                return;
            }

            // LONGEST-CHAIN EXTENSION PATH
            //
            // Normal forward progression of the active branch.
            let Some(located_branch_hash) = branch_hash else {
                match Self::parent_branch_hash_unavailable(block, target, fmt) {
                    Ok(_) => continue,
                    Err(e) => break e,
                }
            };

            // Optimistic mutation:
            //
            // mutate existing branch only
            //
            // no new branch is created here as its carried forward.
            let storage_ref = StorageValueRef::persistent(&located_branch_hash);
            let result = storage_ref.mutate(|result: Result<Option<Branch<T, S>>, _>| {
                let Ok(maybe) = result else {
                    // Decode corruption is treated the same as
                    // missing branch state and enters recovery.
                    match Self::inherited_branch_decode_error(block, target, fmt) {
                        Ok(_) => return Err(None),
                        Err(e) => return Err(Some(e)),
                    }
                };

                let mut branch = match maybe {
                    Some(v) => v,

                    None => match Self::parent_branch_unavailable(block, target, fmt) {
                        Ok(_) => return Err(None),
                        Err(e) => return Err(Some(e)),
                    },
                };

                // Extend active branch head only.
                branch.head = block;

                Ok(branch)
            });

            match result {
                Ok(_) => {}

                Err(e) => match e {
                    // Another OCW won the mutation race.
                    //
                    // Do not retry mutation.
                    //
                    // Promote into a sibling conflict branch
                    // to preserve deterministic execution.
                    // Clone on Mutate Conflict
                    MutateStorageError::ConcurrentModification(_) => {
                        match Self::inherited_branch_mutation_conflict(block, target, fmt) {
                            Ok(_) => {
                                ocw();
                                return;
                            }
                            Err(e) => break e,
                        }
                    }

                    MutateStorageError::ValueFunctionFailed(e) => match e {
                        Some(e) => break e,

                        // Recovery repaired state,
                        // restart cleanly.
                        None => continue,
                    },
                },
            }

            // Persist final routing invariant: block -> divider -> branch
            let block_hash = block_key(Self::TAG, current);

            let Some(divider) = divider_hash else {
                unreachable!()
            };

            store_encoded(&block_hash, &divider);

            // Update longest known boundary only for
            // forward longest-chain progression.
            store_encoded([Self::TAG, HEAD_BLOCK].concat(), &block);

            // Run OCW logic and return
            ocw();
            return;
        };

        return;
    }

    /// Returns the highest known longest-chain boundary used for fork detection.
    ///
    /// This is local fork-tracking state, not consensus finality.
    ///
    /// Used by [`Self::start`] to decide:
    ///
    /// ```ignore
    /// block <= HEAD_BLOCK -> sibling fork path
    /// block >  HEAD_BLOCK -> longest-chain extension
    /// ```
    fn get_head() -> Option<BlockNumberFor<T>> {
        load_value::<BlockNumberFor<T>>(&[Self::TAG, HEAD_BLOCK].concat())
    }

    /// Returns the fork-branch key-hash for a persisted block hash.
    ///
    /// This should be queried using already finalized block hashes,
    /// typically the previous block during OCW execution.
    fn get_branch_hash(hash: T::Hash) -> Option<[u8; 32]> {
        let divider_hash = Self::get_divider(hash)?;
        load_value::<[u8; 32]>(&divider_hash)
    }

    /// Returns the resolved fork-branch data for a persisted block hash.
    ///
    /// This should be queried using already finalized block hashes,
    /// typically the previous block during OCW execution.
    fn get_block_branch(hash: T::Hash) -> Option<Branch<T, S>> {
        let branch_hash = Self::get_branch_hash(hash)?;
        Self::get_branch(&branch_hash)
    }

    /// Loads a branch directly from its branch hash (key).
    fn get_branch(branch_hash: &[u8]) -> Option<Branch<T, S>> {
        load_value::<Branch<T, S>>(branch_hash)
    }

    /// Returns the structural parent branch of a resolved block.
    ///
    /// Useful for manual traversal across fork ancestry.
    ///
    /// This should be queried using already finalized block hashes,
    /// typically the previous block during OCW execution.
    fn get_prev_branch(hash: T::Hash) -> Option<Branch<T, S>> {
        let branch = Self::get_block_branch(hash)?;
        let parent = branch.parent?;
        Self::get_branch(&parent)
    }

    fn get_prev_block_branch() -> Option<Branch<T, S>> {
        let block = Pallet::<T>::block_number();
        let hash = Pallet::<T>::block_hash(block.saturating_sub(One::one()));
        Self::get_block_branch(hash)
    }

    /// Returns the divider for a persisted block hash.
    ///
    /// Divider is the routing layer that allows multiple sibling
    /// branches to coexist from the same parent ancestry.
    ///
    /// This should be queried using already finalized block hashes,
    /// typically the previous block during OCW execution.
    fn get_divider(hash: T::Hash) -> Option<[u8; 32]> {
        let hash = block_key(Self::TAG, hash);
        load_value::<[u8; 32]>(&hash)
    }

    /// Deterministic traversal across persisted local fork branches.
    ///
    /// This moves between already resolved [`Branch`] states created by
    /// [`ForksHandler::start`] during pallet OCW execution.
    ///
    /// Since fork resolution is delayed by one block:
    ///
    /// ```ignore
    /// block N executes
    /// -> block N - 1 is resolved and persisted
    /// ```
    ///
    /// the current executing block is not yet inserted into the local fork
    /// graph during its own OCW execution.
    ///
    /// For normal OCW access, use [`ForksHandler::get_prev_block_branch`]
    /// to retrieve the previous block's (`N - 1`) resolved branch.
    ///
    /// From that branch, navigation can continue using [`ForkAction`] and
    /// [`ForksHandler::transition`], or other helpers such as:
    ///
    /// - [`ForksHandler::get_block_branch`]
    /// - [`ForksHandler::get_prev_branch`]
    /// - [`ForksHandler::get_branch`]
    ///
    /// A branch represents a continuous stream of blocks on the same path:
    ///
    /// ```text
    /// A -> B -> C -> D
    /// same branch
    /// ```
    ///
    /// When a division occurs, a new sibling branch is created:
    ///
    /// ```text
    /// A -> B -> C
    ///         |-- D
    ///         |-- D'
    /// ```
    ///
    /// Here:
    ///
    /// - `D` is the original branch continuation
    /// - `D'` is the new sibling branch
    ///
    /// The branch that existed before the split (`A -> B -> C`)
    /// becomes the parent branch for both paths.
    ///
    /// A child branch is any branch created from that fork point.
    ///
    /// Example:
    ///
    /// ```text
    /// parent branch
    /// A -> B -> C
    ///
    /// child branches
    ///         |-- D   [0]
    ///         |-- D'  [1]
    /// ```
    ///
    /// Nested forks create deeper child branches:
    ///
    /// ```text
    /// A -> B -> C
    ///         |-- D'      [0]
    ///              |-- E' [0,0]
    /// ```
    ///
    /// This function performs branch navigation, not direct block traversal,
    /// so block numbers are not passed here.
    ///
    /// If block position is needed, it can be inferred from:
    ///
    /// - the branch [`Branch::head`]
    /// - the previously resolved branch head
    ///
    /// Traversal uses [`ForkAction`] to move between:
    ///
    /// - parent branches
    /// - child branches
    /// - sibling branches
    /// - root ancestry
    ///
    /// Returns `Some(branch)` if the target exists,
    /// otherwise `None`.
    fn transition(
        branch: &Branch<T, S>,
        action: ForkAction,
    ) -> Option<Branch<T, S>> {
        match action {
 
            ForkAction::MoveToParentBranch => {
                let parent_key = branch.parent?;
                let parent_branch = load_value::<Branch<T, S>>(&parent_key)?;
                Some(parent_branch)
            },
 
            ForkAction::MoveToParentBranchBack(n) => {
                if n.is_zero() {
                    return Some(branch.clone());
                }
                let mut current_branch = branch.clone();
                for _ in 0..n {
                    let next =
                        Self::transition(&current_branch, ForkAction::MoveToParentBranch)?;
                    current_branch = next;
                }
                Some(current_branch)
            },
  

            ForkAction::MoveToChildBranch(index) => {
                let mut child_counter = branch.counter.clone();
                child_counter.push(index);
 
                // branch_key is now called with Self::TAG so the key matches
                // exactly what start() stored - the bug in the old Branch impl
                // was that it omitted the tag entirely.
                let child_key = branch_key(Self::TAG, branch.genesis, &child_counter);
                let child_branch = load_value::<Branch<T, S>>(&child_key)?;
                Some(child_branch)
            },

            ForkAction::MoveToNextChildBranch => {
                Self::transition(branch, ForkAction::MoveToChildBranch(0))
            },
 
  
            ForkAction::MoveToSiblingBranch(index) => {
                let sibling_counter = if branch.counter.is_empty() {
                    // Root branch - children/siblings live at counter [index].
                    vec![index]
                } else {
                    // Replace last element with the target index.
                    let mut c = branch.counter.clone();
                    *c.last_mut()? = index;
                    c
                };
 
                // Bail if the computed counter is identical to the current one
                // (caller asked to move to the branch they are already on).
                if sibling_counter == branch.counter {
                    return None;
                }
 
                let sibling_key = branch_key(Self::TAG, branch.genesis, &sibling_counter);
                let sibling_branch = load_value::<Branch<T, S>>(&sibling_key)?;
                Some(sibling_branch)
            },
 
            ForkAction::MoveToNextSiblingBranch => {
                let next_index = branch.counter.last().map(|k| k.saturating_add(One::one())).unwrap_or(0);
                Self::transition(branch, ForkAction::MoveToSiblingBranch(next_index))
            },
 
            ForkAction::MoveToPreviousSiblingBranch => {
                let last = *branch.counter.last()?;
                if last.is_zero() {
                    return None;
                }
                Self::transition(branch, ForkAction::MoveToSiblingBranch(last.saturating_sub(One::one())))
            },

            ForkAction::MoveToRootBranch => {
                let mut current_branch = branch.clone();
                loop {
                    if current_branch.parent.is_none() {
                        return Some(current_branch);
                    }
                    match Self::transition(&current_branch, ForkAction::MoveToParentBranch) {
                        Some(parent) => {
                            current_branch = parent;
                        },
                        // Parent key was set but branch payload is missing from
                        // storage - return the last successfully loaded branch.
                        None => return Some(current_branch),
                    }
                }
            },
        }
    }

    /// Derives a 32-byte scope key for a given scope item.
    ///
    /// Delegates to [`Accrete::make_key`] which produces a stable, content-addressed
    /// key from the item. 
    /// 
    /// The same item always produces the same key regardless
    /// of the block or fork it is called from, making scope keys fork-independent.
    fn gen_scope_item_key(
        item: &S::Item,
    ) -> [u8; 32] {
        S::make_key(item)
    }

    /// Returns `true` if the given scope key exists in the current fork's branch.
    /// 
    /// Resolves the branch for `block - 1` via [`Self::get_prev_block_branch`]
    /// and checks both the **local** scope (items written on this exact branch)
    /// and the **inherited** scope (items promoted from ancestor branches via
    /// `accrete()`).
    /// 
    /// Returns `Err(OCWForksNotEnabled)` if no branch exists for the previous block, 
    /// which indicates the fork graph has not been initialized via `ForksHandler::start`.
    fn scope_item_exists(   
        key: &[u8; 32],
        target: Option<&str>,
        fmt: Option<LogFormatter<BlockNumberFor<T>, Self::Level>>,
    ) -> Result<bool, Self::Logger> {
        let Some(branch) = Self::get_prev_block_branch() else {
            return Err(<Self as Logging<BlockNumberFor<T>>>::error(
                &Self::forks_not_enabled(),
                Pallet::<T>::block_number(),
                target,
                fmt,
            ));
        };

        if branch.scope.exists_in_local(&key) {
            return Ok(true);
        }

        if branch.scope.exists_in_inherited(&key) {
            return Ok(true);
        }

        Ok(false)
    }

    /// Registers a scope item in the **local** scope of the current branch.
    ///
    /// Resolves the branch for `block - 1` using `block_hash(block - 1)` as
    /// the lookup key. 
    /// 
    /// The item is added to `branch.scope.local_keys` so it is visible to 
    /// [`Self::scope_item_exists`] on the same branch and propagates into 
    /// `inherited_keys` of any child branch created via `accrete()` during
    /// the next `ForksHandler::start` call.
    ///
    /// Returns the 32-byte scope key assigned to the item.
    ///
    /// Returns:
    /// - `Err(OCWForksNotEnabled)` if `block_hash(block - 1)` has no
    /// corresponding branch entry, meaning `ForksHandler::start` has not yet
    /// run at the current block.
    /// - `Err(OCWForksInconsistent)` if the branch hash resolves
    /// but the branch itself cannot be loaded.
    fn add_to_scope(
        item: S::Item,
        target: Option<&str>,
        fmt: Option<LogFormatter<BlockNumberFor<T>, Self::Level>>,
    ) -> Result<[u8; 32], Self::Logger> {
        let block = Pallet::<T>::block_number()
            .saturating_sub(One::one());

        let hash = Pallet::<T>::block_hash(block);

        let Some(branch_hash) = Self::get_branch_hash(hash) else {
            return Err(<Self as Logging<BlockNumberFor<T>>>::error(
                &Self::forks_not_enabled(),
                Pallet::<T>::block_number(),
                target,
                fmt,
            ));
        };

        let Some(mut branch) = Self::get_branch(&branch_hash) else {
            return Err(<Self as Logging<BlockNumberFor<T>>>::error(
                &Self::inconsistent_forks(),
                Pallet::<T>::block_number(),
                target,
                fmt,
            ));
        };

        let key = branch.scope.add_to_local(item);

        store_encoded(&branch_hash, &branch);

        Ok(key)
    }

    /// Removes a scope item from the current branch's local or inherited scope.
    ///
    /// Resolves the branch for `block - 1` and removes the key from whichever
    /// scope layer it occupies:
    ///
    /// - If the key is in `local_keys`, it is removed directly and the branch
    ///   is persisted.
    /// - If the key is in `inherited_keys`, it is removed from the inherited
    ///   layer and the branch is persisted.
    /// - If the key is in neither layer, the call is a no-op and returns `Ok(())`.
    ///
    /// Returns:
    /// - `Err(OCWForksNotEnabled)` if the branch cannot be resolved.
    /// - `Err(OCWForksInconsistent)` if the branch hash exists but the
    /// branch itself cannot be loaded.
    fn remove_from_scope(
        key: &[u8; 32],
        target: Option<&str>,
        fmt: Option<LogFormatter<BlockNumberFor<T>, Self::Level>>,
    ) -> Result<(), Self::Logger> {
        let block = Pallet::<T>::block_number()
            .saturating_sub(One::one());

        let hash = Pallet::<T>::block_hash(block);

        let Some(branch_hash) = Self::get_branch_hash(hash) else {
            return Err(<Self as Logging<BlockNumberFor<T>>>::error(
                &Self::forks_not_enabled(),
                Pallet::<T>::block_number(),
                target,
                fmt,
            ));
        };

        let Some(mut branch) = Self::get_prev_block_branch() else {
            return Err(<Self as Logging<BlockNumberFor<T>>>::error(
                &Self::inconsistent_forks(),
                Pallet::<T>::block_number(),
                target,
                fmt,
            ));
        };

        if branch.scope.exists_in_local(&key) {
            branch.scope.remove_from_local(&key);

            store_encoded(&branch_hash, &branch);

            return Ok(());
        }

        if !branch.scope.exists_in_inherited(&key) {
            return Ok(());
        }

        branch.scope.remove_from_inherited(&key);

        store_encoded(&branch_hash, &branch);

        Ok(())
    }

    /// Returns the error used when fork-aware storage is accessed before
    /// `ForksHandler::start` has initialized the fork graph.
    fn forks_not_enabled() -> DispatchError;

    /// Returns the error used when the fork graph is in an inconsistent state.
    fn inconsistent_forks() -> DispatchError;

    /// Recovery path when a target block's parent block
    /// does not have a resolvable branch available.
    ///
    /// The target block is usually (N-1) and parent (N-2)
    ///
    /// Storage corruption may permanently lose the parent's
    /// local fork scope for that generation, which cannot be
    /// reconstructed from chain history alone.
    ///
    /// Recovery walks backward to the nearest recoverable branch
    /// and restores only the minimum valid scope required for
    /// execution continuity.
    ///
    /// This is intentionally scope-first, not lineage-first:
    /// synthetic recovery branches may be created instead of
    /// exact historical fork reconstruction.
    fn parent_branch_hash_unavailable(
        block: BlockNumberFor<T>,
        _target: Option<&str>,
        _fmt: Option<LogFormatter<BlockNumberFor<T>, Self::Level>>,
    ) -> Result<(), Self::Logger> {
        // Start recovery from: block - 2
        //
        // because the missing branch belongs to: parent(block)
        //
        // and we must search older ancestry for the nearest
        // recoverable branch state.
        let mut recoverer = block.saturating_sub(2u8.into());

        let mut recovered = None;

        // Prevent unbounded historical scanning.
        let mut attempts = 0u32;

        loop {
            // Hard recovery bound reached.
            if attempts >= Self::MAX_RECOVER_TRAVERSAL {
                break;
            }

            // Genesis underflow boundary.
            //
            // Normally unreachable because genesis-adjacent blocks
            // are skipped during regular execution of start().
            //
            // Recovery still handles this path explicitly so synthetic
            // branches can be created for those skipped genesis ranges also.
            if recoverer.is_zero() {
                break;
            }

            // Try resolving: recover_via -> divider -> branch
            //
            // for an older known block.
            let recover_via = Pallet::<T>::block_hash(recoverer.saturating_sub(One::one()));

            // First valid recoverable branch found.
            if let Some(branch) = Self::get_block_branch(recover_via) {
                recovered = Some(branch);
                break;
            }

            // Continue walking backward.
            recoverer = recoverer.saturating_sub(One::one());
            attempts += 1;
        }

        let scope = match recovered {
            // Valid older branch found.
            //
            // Recovery continues from its accreted scope.
            //
            // Any local scope that originally belonged to the
            // missing branch being recovered is permanently lost
            // and cannot be reconstructed from chain history.
            //
            // We therefore continue from the nearest valid
            // recoverable ancestor scope instead.
            Some(branch) => branch.scope.accrete(),

            // No recoverable branch exists.
            //
            // Create a synthetic local branches so execution
            // can continue safely.
            //
            // Example:
            //
            // Real history (lost):
            //
            // A -> B -> C -> D
            //
            // After corruption:
            //
            // A -> ? -> ? -> D
            //
            // Recovery:
            //
            // A -> B
            //
            // C*   D*
            //
            // where:
            //
            // * = synthetic recovery branches
            //
            // Exact lineage is unknown, so recovery restores
            // independent best-case scope accreted roots instead of inventing
            // unverifiable ancestry.
            None => {
                // This path does not recover the full lineage because:
                //
                // - no earlier recoverable branch exists, or
                // - MAX_RECOVER_TRAVERSAL bound was reached, or
                // - block traveral underflowed, indicating genesis blocks for recovery
                //
                // In that case only the immediate parent of the
                // target block is recovered as a synthetic root branch
                // with an empty local scope.

                let parent_block = block.saturating_sub(One::one());
                // Missing parent block being reconstructed.
                let parent = Pallet::<T>::block_hash(parent_block);

                // Used only as deterministic synthetic root anchor.
                let grand_parent = Pallet::<T>::block_hash(block.saturating_sub(2u8.into()));

                // Recovery starts from empty local scope.
                //
                // Exact historical lineage cannot be proven,
                // so we do not attempt full lineage reconstruction.
                //
                // The synthetic root is anchored at the recovery
                // block's parent ancestry:
                //
                //     target block = block
                //     parent       = block - 1 (recover)
                //     grand_parent = block - 2
                //
                // Since we are recovering the missing parent branch,
                // genesis is derived from the parent's parent
                // (`grand_parent`) as the deterministic recovery root.
                let (scope, genesis) = (
                    S::default(),
                    grand_parent.encode().using_encoded(blake2_256),
                );

                let branch = Branch::<T, S> {
                    parent: None,
                    head: parent_block,
                    scope,
                    genesis,
                    counter: Vec::new(),
                };

                // Synthetic independent recovery root.
                let branch_hash = branch_key(Self::TAG, genesis, &[]);

                store_encoded(&branch_hash, &branch);

                // Restore: parent -> divider -> branch
                let divider_hash = divider_key(Self::TAG, parent, branch_hash);

                store_encoded(&divider_hash, &branch_hash);

                let block_hash = block_key(Self::TAG, parent);

                store_encoded(&block_hash, &divider_hash);

                return Ok(());
            }
        };

        // Forward rebuild begins immediately after
        // the last recoverable point.
        let mut target = recoverer.saturating_add(One::one());
        let mut branchkey = None;

        while target < block {
            // Block being synthetically restored.
            let current = Pallet::<T>::block_hash(target);

            let parent = Pallet::<T>::block_hash(target.saturating_sub(One::one()));

            let branch_hash = match branchkey {
                Some(h) => h,
                None => {
                    // Each recovered step is treated as an
                    // independent synthetic scope branch.
                    //
                    // This is intentionally scope-first recovery.
                    //
                    // Any original local scope that belonged to these
                    // historical blocks, if it once existed locally,
                    // is permanently lost and cannot be reconstructed.
                    //
                    // Recovery restores only execution continuity,
                    // not the exact historical fork-local state.

                    let genesis = parent.encode().using_encoded(blake2_256);

                    let branch = Branch::<T, S> {
                        parent: Self::get_branch_hash(Pallet::<T>::block_hash(recoverer.saturating_sub(One::one()))),
                        head: target,
                        scope: scope.clone(),
                        genesis,
                        counter: Vec::new(),
                    };

                    let key = branch_key(Self::TAG, genesis, &[]);

                    store_encoded(&key, &branch);

                    branchkey = Some(key);

                    key
                }
            };

            // Restore routing invariant: block -> divider -> branch
            let divider_hash = divider_key(Self::TAG, parent, branch_hash);

            store_encoded(&divider_hash, &branch_hash);

            let block_hash = block_key(Self::TAG, current);

            store_encoded(&block_hash, &divider_hash);

            target = target.saturating_add(One::one());
        }
        
        Ok(())
    }

    /// Recovery path when a divider exists for a target block's (N-1)
    /// parent (N-2) but the resolved branch data is missing.
    ///
    /// The stale divider is cleared first, then recovery
    /// falls back to [`Self::parent_branch_hash_unavailable`]
    /// to rebuild the minimum valid branch state.
    fn parent_branch_unavailable(
        block: BlockNumberFor<T>,
        target: Option<&str>,
        fmt: Option<LogFormatter<BlockNumberFor<T>, Self::Level>>,
    ) -> Result<(), Self::Logger> {
        // To Resolve: parent -> divider -> branch
        let parent = Pallet::<T>::block_hash(block.saturating_sub(1u8.into()));

        // Divider still exists, so ancestry routing is present.
        //
        // Only the actual branch payload is missing.
        let divider_hash = match Self::get_divider(parent) {
            Some(v) => v,

            // Divider is also missing.
            //
            // This is no longer a branch-only failure and must
            // fall back to full ancestry recovery.
            None => {
                return Self::parent_branch_hash_unavailable(block, target, fmt);
            }
        };

        // Divider points to invalid / missing branch state.
        //
        // Clear stale routing first so recovery can rebuild:
        //
        //      block -> divider -> branch
        //
        // cleanly without reusing corrupted ancestry.
        let mut divider_ref = StorageValueRef::persistent(&divider_hash);

        divider_ref.clear();

        // Delegate to normal branch recovery.
        Self::parent_branch_hash_unavailable(block, target, fmt)
    }

    /// Recovery path when optimistic branch mutation fails due to
    /// concurrent OCW modification.
    ///
    /// Unlike other recovery paths, this handles the target block (N-1)
    /// itself, not its missing parent (N-2) ancestry. It does not return
    /// control to [`Self::start`] for retry, because no parent recovery
    /// is needed.
    ///
    /// Instead, it directly performs branch update for the target
    /// block by cloning the conflicting structure - the inherited
    /// branch from parent into a new sibling branch.
    ///
    /// The conflicting writer keeps the original branch,
    /// while the later writer continues on a cloned sibling fork.
    ///
    /// This preserves deterministic execution without retrying
    /// mutation on already committed branch state.
    fn inherited_branch_mutation_conflict(
        block: BlockNumberFor<T>,
        target: Option<&str>,
        fmt: Option<LogFormatter<BlockNumberFor<T>, Self::Level>>,
    ) -> Result<(), Self::Logger> {
        // Current block being persisted.
        let current = Pallet::<T>::block_hash(block);

        // Parent of the target block.
        // Used to resolve: parent -> divider -> branch(parent)
        //
        // because the conflict belongs to the parent branch lineage.
        let parent = Pallet::<T>::block_hash(block.saturating_sub(One::one()));

        // Locate the existing branch that won the mutation race.
        let branch_hash = match Self::get_branch_hash(parent) {
            Some(v) => v,

            // Parent ancestry is already broken.
            None => {
                return Self::parent_branch_hash_unavailable(block, target, fmt);
            }
        };

        let prev_branch = match Self::get_branch(&branch_hash) {
            Some(v) => v,

            // Divider exists but branch payload is missing.
            None => {
                return Self::parent_branch_unavailable(block, target, fmt);
            }
        };

        // Clone logical scope from the already-existing branch.
        //
        // Conflict resolution becomes:
        //
        // existing branch => sibling branch
        let scope = prev_branch.scope.accrete();
        let genesis = prev_branch.genesis;
        let mut counter = prev_branch.counter.clone();

        // Find the next deterministic sibling slot.
        //
        // Since mutation already happened on the original branch,
        // we must force this execution into a sibling branch.
        //
        // Reusing the same branch would overwrite already
        // committed branch state.
        //
        // Example:
        //
        // Original:
        //
        // A -> B -> C -> D
        //
        // where:
        //
        // A starts the branch lineage
        //
        // If C and D mutate concurrently:
        //
        // writer 1 -> keeps original branch
        // writer 2 -> mutation conflict
        //
        // Recovery becomes:
        //
        // A -> B -> C
        //         |-- C' -> D
        //
        // We effectively abandon continuation on the original
        // branch and create a cloned sibling fork from the
        // last safe branch point.
        //
        // The cloned branch keeps:
        //
        // - same scope lineage
        // - same genesis
        //
        // and only receives a new sibling counter:
        //
        // [0], [1], [2], ...
        let mut i = 0u32;

        let next_counter = loop {
            let mut try_counter = counter.clone();
            try_counter.push(i);

            let try_branch_hash = branch_key(Self::TAG, genesis, &try_counter);

            // First empty sibling slot found.
            if load_value::<Branch<T, S>>(&try_branch_hash).is_none() {
                break Some(i);
            }

            i += 1;

            if i > Self::MAX_FORKS {
                break None;
            }
        };

        let Some(new_counter) = next_counter else {
            return Self::max_forks(block, target, fmt);
        };

        counter.push(new_counter);

        // New deterministic sibling branch identity.
        let new_branch_hash = branch_key(Self::TAG, genesis, &counter);

        let new_branch = Branch::<T, S> {
            parent: Some(branch_hash),
            head: block,
            scope,
            genesis,
            counter,
        };

        store_encoded(&new_branch_hash, &new_branch);

        // New sibling divider: parent + branch
        let divider_hash = divider_key(Self::TAG, parent, new_branch_hash);

        store_encoded(&divider_hash, &new_branch_hash);

        // Final resolution: block -> divider -> branch
        let block_hash = block_key(Self::TAG, current);

        store_encoded(&block_hash, &divider_hash);

        // Conflict branch becomes the new reachable head.
        store_encoded([Self::TAG, HEAD_BLOCK].concat(), &block);

        Ok(())
    }

    /// Recovery path when divider routing is missing for the
    /// target block's (N-1) parent (N-2).
    ///
    /// Since divider loss breaks ancestry routing entirely,
    /// recovery falls back directly to
    /// [`Self::parent_branch_hash_unavailable`].
    fn parent_divider_unavailable(
        block: BlockNumberFor<T>,
        target: Option<&str>,
        fmt: Option<LogFormatter<BlockNumberFor<T>, Self::Level>>,
    ) -> Result<(), Self::Logger> {
        Self::parent_branch_hash_unavailable(block, target, fmt)
    }

    /// Recovery path when branch decoding fails for the
    /// target block's (N-1) inherited branch from parent (N-2)
    /// for extension.
    ///
    /// This means the branch exists in storage,
    /// but its payload is corrupted or unreadable.
    ///
    /// It is treated the same as a missing branch and
    /// delegated to [`Self::parent_branch_unavailable`].
    fn inherited_branch_decode_error(
        block: BlockNumberFor<T>,
        target: Option<&str>,
        fmt: Option<LogFormatter<BlockNumberFor<T>, Self::Level>>,
    ) -> Result<(), Self::Logger> {
        Self::parent_branch_unavailable(block, target, fmt)
    }

    /// Triggered when no additional sibling branch slot
    /// can be allocated under the configured limit as the sibling
    /// of the target block's (N-1) parent (N-2).
    ///
    /// This occurs when:
    ///
    /// ```ignore
    /// sibling_count > MAX_FORKS
    /// ```
    ///
    /// and branch creation must stop.
    fn max_forks(
        block: BlockNumberFor<T>,
        target: Option<&str>,
        fmt: Option<LogFormatter<BlockNumberFor<T>, Self::Level>>,
    ) -> Result<(), Self::Logger> {
        Err(<Self as Logging<BlockNumberFor<T>>>::error(
            &Self::max_forks_error(),
            block,
            target,
            fmt,
        ))
    }

    /// Error returned when fork creation exceeds
    /// [`Self::MAX_FORKS`].
    fn max_forks_error() -> DispatchError;

}

// ===============================================================================
// `````````````````````````````` PRIVATE UTILITIES ``````````````````````````````
// ===============================================================================

/// Deterministic storage hash builder.
///
/// Used for all Persistent Node-Local storage keys.
fn make_hash(tag: &[u8], input: impl Encode, suffix: &[u8]) -> [u8; 32] {
    let mut source = tag.encode();
    source.extend_from_slice(&input.encode());
    source.extend_from_slice(suffix);
    blake2_256(&source)
}

/// Persist an encoded value into Persistent Node-Local storage.
fn store_encoded<K: AsRef<[u8]>, V: Encode>(key: K, value: &V) {
    let storage_ref = StorageValueRef::persistent(key.as_ref());
    storage_ref.set(&value);
}

/// Load and decode a value from Persistent Node-Local storage.
fn load_value<V: codec::Decode>(key: &[u8]) -> Option<V> {
    let storage_ref = StorageValueRef::persistent(key);
    let Ok(result) = storage_ref.get::<V>() else {
        return None;
    };
    result
}

/// Deterministic branch identity key
///
/// ```ignore
/// genesis + counter lineage -> branchkey
/// ```
fn branch_key(tag: &[u8], genesis: [u8; 32], counter: &[u32]) -> [u8; 32] {
    let mut identity = genesis.encode();

    for c in counter {
        identity.extend_from_slice(&c.encode());
    }

    make_hash(tag, &identity, b"branch")
}

/// Divider identity key
///
/// ```ignore
/// parent + branch -> divider
/// ```
///
/// Allows sibling branches from the same parent.
fn divider_key(tag: &[u8], hash: impl Encode, branch_key: [u8; 32]) -> [u8; 32] {
    let mut identity = hash.encode();
    identity.extend_from_slice(&branch_key.encode());

    make_hash(tag, &identity, b"divider")
}

/// Block routing identity key
///
/// ```ignore
/// block -> divider
/// ```
fn block_key(tag: &[u8], hash: impl Encode) -> [u8; 32] {
    let identity = hash.encode();
    make_hash(tag, &identity, b"block")
}