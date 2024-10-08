//! Implementation of `VESTING_WALLET`
//!
//! Handles the vesting of Ether and ERC20 tokens for a given beneficiary
//! Custody of multiple tokens can be given to this contract,
//! which will release the token to the beneficiary following a given,
//! customizable, vesting schedule
//!
//! This contract has referenced the `OpenZeppelin` vesting_wallet
//! implementation guidelines
//! [VestingWallet]
//! This contract module inherits `ERC-20` and `Ownable` contracts

extern crate alloc;

use alloy_primitives::{Address, U256, U64};
use alloy_sol_types::sol;
use openzeppelin_stylus_proc::interface_id;
use stylus_sdk::{
    prelude::storage,
    storage::{StorageMap, StorageU64},
};
use alloc::vec::Vec;
use stylus_sdk::prelude::SolidityError;
use stylus_sdk::storage::StorageU256;


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
    /// Error returned when the beneficiary cannot claim the tokens, as the schedule does not allow
    /// at that particular time.
    ///
    /// This error is associated with `release_eth`
    #[derive(Debug)]
    #[allow(missing_docs)]
    error CannotReleaseETh();

    /// Error returned when the beneficiary cannot claim the tokens, as the schedule does not allow
    /// at that particular time.
    ///
    /// This error is associated with `release_erc20`
    #[derive(Debug)]
    #[allow(missing_docs)]
    error CannotReleaseERC20();
}

/// An Error Type defined for [VestingWallet]
#[derive(SolidityError,Debug)]
pub enum Error {
    /// This error is associated with `release_eth`
    CannotReleaseEth(CannotReleaseETh),
    /// This error is associated with `release_erc20`
    CannotReleaseERC20(CannotReleaseERC20)
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
}

/// Trait `Vesting` defines all necessary vesting functionality per
/// `OpenZeppelin` solidity implementation
/// For extensibility purposes the name has a suffix indicating the versioning
/// i.e `VestingV1` indicating version 1
#[interface_id]
pub trait VestingV1 {
    type Error: Into<Vec<u8>>;
    fn receive_eth() -> Result<(),Vec<u8>>;
    fn receive_erc20_token(token: Address) -> Result<(),Vec<u8>>;

    fn release_eth() -> Result<(),Self::Error>;

    fn release_erc20(token:Address) -> Result<(),Self::Error>;

    fn vested_eth_amount() -> U256;

    fn vested_erc20_amount(token:Address) -> U256;

    fn vesting_schedule(total_alloc: U256, time: U64) -> U256;
}
