#![cfg_attr(not(test), no_main, no_std)]

extern crate alloc;
use alloc::vec::Vec;

use alloy_primitives::Address;
use openzeppelin_stylus::{
    access::ownable::Ownable,
    finance::vesting_wallet::{IVesting, VestingWallet},
};
use stylus_sdk::{
    msg,
    prelude::{entrypoint, public, storage},
};

#[storage]
#[entrypoint]
pub struct VestingWalletExample {
    #[borrow]
    vesting_wallet: VestingWallet,
    #[borrow]
    ownable: Ownable,
}

#[public]
#[inherit(VestingWallet, Ownable)]
impl VestingWalletExample {
    /// Overrides the current [`VestingWallet::release_eth`] implementation,
    ///
    /// Adds checking if the owner is the caller of the function.
    /// limiting access only to the owner of the contract
    /// no-op if the caller is not the owner
    #[selector(name = "release")]
    pub fn release_eth(&mut self) -> Result<(), Vec<u8>> {
        let caller = msg::sender();
        if caller != self.vesting_wallet.ownable.owner() {
            return Ok(());
        }
        self.vesting_wallet.release_eth()
    }

    /// Overrides the current [`VestingWallet::release_erc20`] implementation
    ///
    /// Adds checking if the owner is the caller of the function.
    /// limiting access only to the owner of the contract
    /// no-op if the caller is not the owner
    #[selector(name = "release")]
    pub fn release_erc20(&mut self, token: Address) -> Result<(), Vec<u8>> {
        let caller = msg::sender();
        if caller != self.vesting_wallet.ownable.owner() {
            return Ok(());
        }

        self.vesting_wallet.release_erc20(token).map_err(|err| err.into())
    }
}
