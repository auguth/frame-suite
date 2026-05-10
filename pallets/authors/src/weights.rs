
#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

/// Weight functions needed for `pallet_authors`.
pub trait WeightInfo {
fn enlist() -> Weight;
fn demit() -> Weight;
fn refill() -> Weight;
fn my_collateral() -> Weight;
fn direct_fund() -> Weight;
fn index_fund() -> Weight;
fn pool_fund() -> Weight;
fn release_direct_fund() -> Weight;
fn release_index_fund() -> Weight;
fn release_pool_fund() -> Weight;
fn confirm() -> Weight;
fn create_index() -> Weight;
fn create_pool() -> Weight;
fn transfer_pool() -> Weight;
fn update_commission() -> Weight;
fn update_slot_shares() -> Weight;
fn update_entry_shares() -> Weight;
fn force_probation_period() -> Weight;
fn force_reduce_probation_by() -> Weight;
fn force_increase_probation_by() -> Weight;
fn force_rewards_buffer() -> Weight;
fn force_penalties_buffer() -> Weight;
fn force_max_elected() -> Weight;
fn force_min_elected() -> Weight;
fn force_enforce_max_elected() -> Weight;
fn force_min_fund() -> Weight;
fn force_max_exposure() -> Weight;
fn force_min_collateral() -> Weight;
fn check_direct_fund() -> Weight;
fn check_index_fund() -> Weight;
fn check_index_fund_towards() -> Weight;
fn check_pool_fund() -> Weight;
fn check_pool_fund_towards() -> Weight;
fn shed_rewards() -> Weight;
fn shed_penalties() -> Weight;
fn on_initialize_rewards_penalties(r: u32, p: u32, ) -> Weight;
}

impl WeightInfo for () {
    fn enlist() -> Weight { Weight::zero() }
    fn demit() -> Weight { Weight::zero() }
    fn refill() -> Weight { Weight::zero() }
    fn my_collateral() -> Weight { Weight::zero() }
    fn direct_fund() -> Weight { Weight::zero() }
    fn index_fund() -> Weight { Weight::zero() }
    fn pool_fund() -> Weight { Weight::zero() }
    fn release_direct_fund() -> Weight { Weight::zero() }
    fn release_index_fund() -> Weight { Weight::zero() }
    fn release_pool_fund() -> Weight { Weight::zero() }
    fn confirm() -> Weight { Weight::zero() }
    fn create_index() -> Weight { Weight::zero() }
    fn create_pool() -> Weight { Weight::zero() }
    fn transfer_pool() -> Weight { Weight::zero() }
    fn update_commission() -> Weight { Weight::zero() }
    fn update_slot_shares() -> Weight { Weight::zero() }
    fn update_entry_shares() -> Weight { Weight::zero() }
    fn force_probation_period() -> Weight { Weight::zero() }
    fn force_reduce_probation_by() -> Weight { Weight::zero() }
    fn force_increase_probation_by() -> Weight { Weight::zero() }
    fn force_rewards_buffer() -> Weight { Weight::zero() }
    fn force_penalties_buffer() -> Weight { Weight::zero() }
    fn force_max_elected() -> Weight { Weight::zero() }
    fn force_min_elected() -> Weight { Weight::zero() }
    fn force_enforce_max_elected() -> Weight { Weight::zero() }
    fn force_min_fund() -> Weight { Weight::zero() }
    fn force_max_exposure() -> Weight { Weight::zero() }
    fn force_min_collateral() -> Weight { Weight::zero() }
    fn check_direct_fund() -> Weight { Weight::zero() }
    fn check_index_fund() -> Weight { Weight::zero() }
    fn check_index_fund_towards() -> Weight { Weight::zero() }
    fn check_pool_fund() -> Weight { Weight::zero() }
    fn check_pool_fund_towards() -> Weight { Weight::zero() }
    fn shed_rewards() -> Weight { Weight::zero() }
    fn shed_penalties() -> Weight { Weight::zero() }

    fn on_initialize_rewards_penalties(_r: u32, _p: u32) -> Weight {
        Weight::zero()
    }
}