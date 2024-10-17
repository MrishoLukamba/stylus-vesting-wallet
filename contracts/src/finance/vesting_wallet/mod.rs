//! Implementation of `VESTING_WALLET`
//!
//! Handles the vesting of Ether and ERC20 tokens for a given beneficiary
//! Custody of multiple tokens can be given to this contract,
//! which will release the token to the beneficiary following a given,
//! customizable, vesting schedule
//!
//! This contract has referenced the `OpenZeppelin` `vesting_wallet`
//! implementation guidelines
//! [`VestingWallet`]
//! This contract module depends `ERC-20`[`crate::token::erc20`] and
//! `Ownable`[`crate::access::ownable`] contracts

extern crate alloc;

use alloc::vec::Vec;
mod erc20;
use alloy_primitives::{uint, Address, U256, U64};
use alloy_sol_types::sol;
use erc20::Erc20;
use stylus_sdk::{
    block,
    call::{transfer_eth, Call},
    contract, evm, function_selector,
    prelude::{public, storage, SolidityError, TopLevelStorage},
    storage::{StorageMap, StorageU256, StorageU64},
};

use crate::{
    access::ownable::Ownable, utils::math::storage::AddAssignUnchecked,
};

sol! {
  /// Event emitted when ETHER token is being released and transferred from contract(`this`) to benefeciary
  /// account(`benef`), tracking `amount` released and `beneficiary` account
  ///
  /// Note that this is done after calling `release_eth` method
  #[allow(missing_docs)]
  event EtherReleased(address indexed beneficiary, uint256 value);
  /// Event emitted when ERC-20 token is being released and transferred from contract(`this`) to benefeciary
  /// account(`benef`), tracking `amount` released and `beneficiary` account
  ///
  /// Note that this is done after calling `release_token` method
  #[allow(missing_docs)]
  event ERC20Released(address indexed beneficiary, address indexed token, uint256 value);
}

sol! {

    /// Error returned when remote contract call fails
    #[derive(Debug)]
    #[allow(missing_docs)]
    error RemoteContractCallFailed();
}

/// [`VestingWallet`] error
#[derive(SolidityError, Debug)]
pub enum Error {
    /// Error returned when contract call fails, i.e reading contract value,
    /// sending Erc20 tokens
    RemoteContractCallFailed(RemoteContractCallFailed),
}

/// State of Vesting_Wallet
#[storage]
pub struct VestingWallet {
    /// Total Ether tokens released from the contract
    eth_released: StorageU256,
    /// Total amount ERC-20 tokens released from the contract
    erc20_released: StorageMap<Address, StorageU256>,
    /// Start timestamp set initially, Note this is immutable after being set
    start: StorageU64,
    /// Duration set initially for the vesting, Note this is immutable after
    /// being set
    duration: StorageU64,
    /// access to ownable contract [crate::access::ownable]
    pub ownable: Ownable,
}

/// Trait `IVesting` defines all necessary vesting functionality per
/// `OpenZeppelin` solidity implementation
pub trait IVesting {
    /// The contract should be able to receive ether token
    ///
    /// # Arguments
    ///
    ///  * `&mut self` - allowing mutating contract account balance state
    fn receive(&mut self);

    /// Beneficiary will call this function to receive vested ether tokens
    ///
    /// Gets the amount of releasable eth, updates the `eth_released` state and
    /// calls `transfer_eth` with `owner` as the beneficiary and `amount` of
    /// released eth per the `timestamp`
    ///
    /// # Arguments
    ///
    ///  * `&mut self` - allowing mutating contract and beneficiary account
    ///    balance state
    ///
    /// # Event
    ///
    /// Emits [EtherReleased] event
    ///
    /// # Errors
    ///
    /// Returns encoded error type defined on `transfer_eth` function
    fn release_eth(&mut self) -> Result<(), Vec<u8>>;

    /// Beneficiary will call this function to receive vested ERC-20 tokens
    ///
    /// Gets the amount of releasable erc20, updates the `erc20_released` state
    /// and constructs a `ERC20::transfer` remote call with `owner` as the
    /// beneficiary and `amount` of released Erc20 token per the `timestamp`
    ///
    /// # Arguments
    ///
    ///  * `&mut self` - allowing mutating contract and beneficiary account
    ///    balance state
    ///  * `token` - specifying which ERC-20 token address to release
    ///
    /// # Event
    ///
    /// Emits [ERC20Released] event
    ///
    /// # Errors
    ///
    /// Returns [Error::RemoteContractCallFailed] if it fails to send ERC20
    /// token to beneficiary
    fn release_erc20(&mut self, token: Address) -> Result<(), Error>;

    /// Calculates the amount of ether that has already vested.
    /// Default implementation is a linear vesting curve.
    ///
    /// # Arguments
    ///
    /// * `timestamp` - block timestamp
    fn vested_eth_amount(&self, timestamp: u64) -> U256;

    /// Calculates the amount of ERC-20 that has already vested.
    /// Default implementation is a linear vesting curve.
    ///
    /// Gets the contract's Erc20 balance by constructing a `Erc20::balance_of`
    /// remote call
    ///
    /// # Arguments
    ///
    /// * `token` - Erc20 contract address
    /// * `timestamp` -  block timestamp
    ///
    /// # Errors
    ///
    /// * returns [RemoteContractCallFailed] if the remote call fails
    fn vested_erc20_amount(
        &mut self,
        token: Address,
        timestamp: u64,
    ) -> Result<U256, Error>;

    /// Getter for the start timestamp.
    fn start(&self) -> U256;

    /// Getter for the vesting duration.
    fn duration(&self) -> U256;

    /// Getter for the end timestamp.
    fn end(&self) -> U256;

    /// Amount of eth already released
    fn released_eth(&self) -> U256;

    /// Amount of ERC-20 token already released
    ///
    /// # Argument
    ///
    /// * `token` - ERC-20 token address
    fn released_erc20(&self, token: Address) -> U256;

    /// Getter for the amount of releasable eth.
    fn releasable_eth(&self) -> U256;

    /// Getter for the amount of releasable ERC-20 token.
    ///
    /// # Arguments
    ///
    ///  * `token` - specifying ERC-20 token contract address
    fn releasable_erc20(&mut self, token: Address) -> Result<U256, Error>;

    /// Re-exporting `Ownable` contract functions for easier accessing

    /// Returns the address of the current owner.
    fn owner(&self) -> Address;

    /// Checks if the `msg::sender` is set as the owner.
    ///
    /// # Errors
    ///
    /// If called by any account other than the owner, then the error
    /// [`crate::access::ownable::Error::UnauthorizedAccount`] is returned.
    fn only_owner(&self) -> Result<(), crate::access::ownable::Error>;

    /// Transfers ownership of the contract to a new account (`new_owner`). Can
    /// only be called by the current owner.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `new_owner` - The next owner of this contract.
    ///
    /// # Errors
    ///
    /// If `new_owner` is the zero address, then the error
    /// `OwnableInvalidOwner` is returned.
    fn transfer_ownership(
        &mut self,
        new_owner: Address,
    ) -> Result<(), crate::access::ownable::Error>;

    /// Leaves the contract without owner. It will not be possible to call
    /// [`Self::only_owner`] functions. Can only be called by the current owner.
    ///
    /// NOTE: Renouncing ownership will leave the contract without an owner,
    /// thereby disabling any functionality that is only available to the owner.
    ///
    /// # Errors
    ///
    /// If not called by the owner, then the error
    /// [`crate::access::ownable::Error::UnauthorizedAccount`] is returned.
    fn renounce_ownership(
        &mut self,
    ) -> Result<(), crate::access::ownable::Error>;
}

unsafe impl TopLevelStorage for VestingWallet {}

#[public]
impl IVesting for VestingWallet {
    #[payable]
    fn receive(&mut self) {}

    #[selector(name = "release")]
    fn release_eth(&mut self) -> Result<(), Vec<u8>> {
        self._release_eth()
    }

    #[selector(name = "release")]
    fn release_erc20(&mut self, token: Address) -> Result<(), Error> {
        self._release_erc20(token)
    }

    #[selector(name = "vestedAmount")]
    fn vested_eth_amount(&self, timestamp: u64) -> U256 {
        let balance = contract::balance();
        // SAFETY: cannot panic, as timestamp is always u64;
        let timestamp = U64::try_from(timestamp).unwrap();
        self.vesting_schedule(balance + self.released_eth(), timestamp)
    }

    #[selector(name = "vestedAmount")]
    fn vested_erc20_amount(
        &mut self,
        token: Address,
        timestamp: u64,
    ) -> Result<U256, Error> {
        self._vested_erc20_amount(token, timestamp)
    }

    fn start(&self) -> U256 {
        self.start.get().to()
    }

    fn duration(&self) -> U256 {
        self.duration.get().to()
    }

    fn end(&self) -> U256 {
        let duration: U256 = self.duration.get().to();
        // SAFETY: cannot overflow
        self.start() + duration
    }

    #[selector(name = "released")]
    fn released_eth(&self) -> U256 {
        *self.eth_released
    }

    #[selector(name = "released")]
    fn released_erc20(&self, token: Address) -> U256 {
        self.erc20_released.get(token)
    }

    #[selector(name = "releasable")]
    fn releasable_eth(&self) -> U256 {
        let timestamp = block::timestamp();
        self.vested_eth_amount(timestamp) - self.released_eth()
    }

    #[selector(name = "releasable")]
    fn releasable_erc20(&mut self, token: Address) -> Result<U256, Error> {
        let timestamp = block::timestamp();
        let vested_erc20 = self.vested_erc20_amount(token, timestamp)?;
        Ok(vested_erc20 - self.released_erc20(token))
    }

    // ======================================================= //
    fn owner(&self) -> Address {
        self.ownable.owner()
    }

    /// Checks if the `msg::sender` is set as the owner.
    ///
    /// # Errors
    ///
    /// If called by any account other than the owner, then the error
    /// [`crate::access::ownable::Error::UnauthorizedAccount`] is returned.
    fn only_owner(&self) -> Result<(), crate::access::ownable::Error> {
        self.ownable.only_owner()
    }

    /// Transfers ownership of the contract to a new account (`new_owner`). Can
    /// only be called by the current owner.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `new_owner` - The next owner of this contract.
    ///
    /// # Errors
    ///
    /// If `new_owner` is the zero address, then the error
    /// `OwnableInvalidOwner` is returned.
    fn transfer_ownership(
        &mut self,
        new_owner: Address,
    ) -> Result<(), crate::access::ownable::Error> {
        self.ownable.transfer_ownership(new_owner)
    }

    /// Leaves the contract without owner. It will not be possible to call
    /// [`Self::only_owner`] functions. Can only be called by the current owner.
    ///
    /// NOTE: Renouncing ownership will leave the contract without an owner,
    /// thereby disabling any functionality that is only available to the owner.
    ///
    /// # Errors
    ///
    /// If not called by the owner, then the error
    /// [`crate::access::ownable::Error::UnauthorizedAccount`] is returned.
    fn renounce_ownership(
        &mut self,
    ) -> Result<(), crate::access::ownable::Error> {
        self.ownable.renounce_ownership()
    }
}

impl VestingWallet {
    /// Virtual implementation of the vesting formula.
    /// This returns the amount vested, as a function of time,
    /// for an asset given its total historical allocation.
    ///
    /// **Arguments**
    ///
    ///  * `total_alloc` - total amount allocated
    ///  * `timestamp` - current block timestamp
    fn vesting_schedule(&self, total_alloc: U256, timestamp: U64) -> U256 {
        let timestamp = timestamp.to::<U256>();

        if timestamp < self.start() {
            uint!(0_U256)
        } else if timestamp >= self.end() {
            total_alloc
        } else {
            // calculate the elapsed time as a fraction of duration and
            // multiplying it by the allocated amount.
            (total_alloc * (timestamp - self.start()).to::<U256>())
                / self.duration()
        }
    }

    /// Internal implementation of `release_eth`
    fn _release_eth(&mut self) -> Result<(), Vec<u8>> {
        let amount = self.releasable_eth();
        // SAFETY: cannot overflow, it is unreleastic the balance overflowing
        // U256::MAX
        self.eth_released.checked_add(amount).unwrap();

        let owner = self.ownable.owner();
        // SAFETY: transfer cannot fail;
        transfer_eth(owner, amount)?;
        evm::log(EtherReleased { beneficiary: owner, value: amount });
        Ok(())
    }

    /// Internal implementation of `release_erc20`
    fn _release_erc20(&mut self, token: Address) -> Result<(), Error> {
        let amount = self.releasable_erc20(token)?;
        // SAFETY: cannot overflow, it is unreleastic the balance overflowing
        // U256::MAX
        self.erc20_released.setter(token).add_assign_unchecked(amount);

        // remote Erc20 contract transfer call
        let owner = self.ownable.owner();
        let erc20_interactor = Erc20::new(token);
        let call = Call::new_in(self);

        let result =
            erc20_interactor.transfer(call, owner, amount).map_err(|_err| {
                Error::RemoteContractCallFailed(RemoteContractCallFailed {})
            })?;
        if !result {
            Err(Error::RemoteContractCallFailed(RemoteContractCallFailed {}))?
        }
        evm::log(ERC20Released { beneficiary: owner, token, value: amount });
        Ok(())
    }

    /// Internal implementation of `vested_erc20_amount`
    fn _vested_erc20_amount(
        &mut self,
        token: Address,
        timestamp: u64,
    ) -> Result<U256, Error> {
        // remote ERC20 contract balance_of call
        let balance: U256 = {
            let erc20_instance = Erc20::new(token);
            let result = erc20_instance
                .balance_of(&mut *self, contract::address())
                .map_err(|_err| {
                    Error::RemoteContractCallFailed(RemoteContractCallFailed {})
                })?;
            result
        };

        // SAFETY: cannot panic, as timestamp is always u64;
        let timestamp = U64::try_from(timestamp).unwrap();
        Ok(self
            .vesting_schedule(balance + self.released_erc20(token), timestamp))
    }
}

// ============================================================================
// Unit Motsu Tests: Vesting Wallet
// ============================================================================
#[cfg(all(test, feature = "std"))]
mod tests {
    use alloy_primitives::{address, uint, U256, U64};
    use motsu::prelude::*;
    use stylus_sdk::{call::transfer_eth, contract};

    use super::block;
    use crate::{
        finance::vesting_wallet::{IVesting, VestingWallet},
        token::erc20::{Erc20, IErc20},
    };

    // helper macro
    macro_rules! assert_ok {
        ($func:expr) => {
            let func_return = $func.unwrap();
            assert_eq!(func_return, ())
        };
    }

    // beneficiary

    #[motsu::test]
    fn contract_receiving_erc20_works() {
        let mut erc_20 = Erc20::default();

        let vesting_addr = contract::address();
        let amount = uint!(10_000_U256);
        // mint tokens to vesting contract
        assert_ok!(erc_20._mint(vesting_addr, amount));

        let balance = erc_20.balance_of(vesting_addr);

        assert_eq!(balance, amount)
    }

    #[motsu::test]
    fn contract_receiving_ether_works(_contract: VestingWallet) {
        // fund vesting wallet contract eth balance
        let vesting_addr = contract::address();
        let amount = uint!(10_000_U256);
        assert_ok!(transfer_eth(vesting_addr, amount));

        // NOTE: we cannot test contract eth balance after transfer_eth()
        // error returned: dyld[74269]: missing symbol called when calling
        // contract::balance(); no function in motsu::shim
        // assert_eq!(amount,contrac&self,call: Call<()>, account: Address// but
        // it will be covered in e2e tests
    }

    #[motsu::test]
    fn getters_work(contract: VestingWallet) {
        let start = uint!(1_U64);
        let duration = uint!(9_U64);

        contract.start.set(start);
        contract.duration.set(duration);

        assert_eq!(contract.start(), start.to::<U256>());
        assert_eq!(contract.duration(), duration.to::<U256>());
        assert_eq!(contract.end(), (start + duration).to::<U256>());

        let eth_released = uint!(100_U256);
        let token1_released = uint!(100_U256);
        let token2_released = uint!(100_U256);
        let alice_token = address!("A11CEacF9aa32246d767FCCD72e02d6bCbcC375d");
        let bob_token = address!("B0B0cB49ec2e96DF5F5fFB081acaE66A2cBBc2e2");

        contract.eth_released.set(eth_released);
        contract.erc20_released.insert(alice_token, token1_released);
        contract.erc20_released.insert(bob_token, token2_released);

        assert_eq!(contract.released_eth(), eth_released);
        assert_eq!(contract.released_erc20(alice_token), token1_released);
        assert_eq!(contract.released_erc20(bob_token), token2_released);
    }

    // test case where, timestamp > self::end()
    #[motsu::test]
    fn vesting_schedule_timestamp_less_than_start(contract: VestingWallet) {
        let timestamp = U64::try_from(block::timestamp()).unwrap();

        let start = timestamp + uint!(1_U64);
        contract.start.set(start);

        let vested_amount =
            contract.vesting_schedule(uint!(100_000_U256), timestamp);
        assert_eq!(vested_amount, uint!(0_U256))
    }

    // test case where, timestamp > self::end()
    #[motsu::test]
    fn vesting_schedule_end_less_than_timestamp(contract: VestingWallet) {
        let timestamp = U64::try_from(block::timestamp()).unwrap();
        let amount = uint!(100_000_U256);

        let start = timestamp - uint!(10_000_U64);
        let duration = uint!(5_000_U64);
        contract.start.set(start);
        contract.duration.set(duration);

        let vested_amount = contract.vesting_schedule(amount, timestamp);
        assert_eq!(vested_amount, amount)
    }

    #[motsu::test]
    fn linear_vesting_schedule_works(contract: VestingWallet) {
        // testing one year linear vesting, should return fraction of total
        // allocation the schedule tested quarterly

        let timestamp = U64::try_from(block::timestamp()).unwrap();
        // amount in wei
        let amount = uint!(100_000_U256) * uint!(10_U256).pow(uint!(18_U256));

        // set start time to be a quarter year before current timestamp
        let start = timestamp - uint!(5_929_200_U64);
        // set duration to be start + duration = year
        let duration = uint!(23_716_800_U64);

        contract.start.set(start);
        contract.duration.set(duration);

        // 1st quarter elapsed (exactly one quarter year)
        let vested_amount_q1 =
            contract.vesting_schedule(amount, start + uint!(5_929_200_U64));
        let expected_q1 =
            uint!(25_000_U256) * uint!(10_U256).pow(uint!(18_U256));
        assert_eq!(vested_amount_q1, expected_q1); // 25% vested

        // 2nd quarter elapsed (half the year)
        let vested_amount_q2 =
            contract.vesting_schedule(amount, start + uint!(11_858_400_U64));
        let expected_q2 =
            uint!(50_000_U256) * uint!(10_U256).pow(uint!(18_U256));
        assert_eq!(vested_amount_q2, expected_q2); // 50% vested

        // 3rd quarter elapsed (three-quarters of the year)
        let vested_amount_q3 =
            contract.vesting_schedule(amount, start + uint!(17_787_600_U64));
        let expected_q3 =
            uint!(75_000_U256) * uint!(10_U256).pow(uint!(18_U256));
        assert_eq!(vested_amount_q3, expected_q3); // 75% vested

        // Full year elapsed (all vested)
        let vested_amount_q4 =
            contract.vesting_schedule(amount, start + uint!(23_716_800_U64));
        let expected_q4 =
            uint!(100_000_U256) * uint!(10_U256).pow(uint!(18_U256));
        assert_eq!(vested_amount_q4, expected_q4); // 100% vested
    }

    #[motsu::test]
    fn duration_zero_act_as_time_lock_contract_vesting_schedule(
        contract: VestingWallet,
    ) {
        let timestamp = U64::try_from(block::timestamp()).unwrap();
        let start = timestamp + uint!(5_929_200_U64);
        let amount = uint!(100_000_U256) * uint!(10_U256).pow(uint!(18_U256));

        contract.start.set(start);
        contract.duration.set(uint!(0_U64));
        // lock funds until timestamp >= start, and release all funds without
        // vesting
        let vested_amount = contract.vesting_schedule(amount, timestamp);
        assert_eq!(vested_amount, uint!(0_U256));
        // time elapsed
        let new_timestamp = start;
        let vested_amount = contract.vesting_schedule(amount, new_timestamp);
        assert_eq!(vested_amount, amount);
    }

    #[motsu::test]
    fn vested_eth_amount_works(contract: VestingWallet) {
        let timestamp = block::timestamp();
        let start = timestamp - 5_929_200_u64;
        let duration = uint!(23_716_800_U64);
        let _amount = uint!(100_000_U256) * uint!(10_U256).pow(uint!(18_U256));

        contract.start.set(U64::try_from(start).unwrap());
        contract.duration.set(duration);

        contract.eth_released.set(uint!(0_U256));
        let _expected_q1 =
            uint!(25_000_U256) * uint!(10_U256).pow(uint!(18_U256));

        // assert_eq!(contract.vested_eth_amount(start +
        // 5_929_200_u64),expected_q1); cannot compile due to
        // contract::balance() not available in the SHIM this will be
        // tested in e2e tests
    }
}
