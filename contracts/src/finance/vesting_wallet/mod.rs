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
//! This contract module depends `ERC-20`[crate::token::erc20] and `Ownable`[crate::access::ownable] contracts

extern crate alloc;

use crate::access::ownable::Ownable;
use alloc::vec::Vec;
use alloy_primitives::{uint, Address, U256, U64};
use alloy_sol_types::sol_data::Uint;
use alloy_sol_types::{sol, SolType};
use ethabi::Token;
use openzeppelin_stylus_proc::interface_id;
use stylus_sdk::call::{call, static_call, transfer_eth, Call};
use stylus_sdk::prelude::{public, SolidityError, TopLevelStorage};
use stylus_sdk::{
    block, contract, evm, function_selector,
    prelude::storage,
    storage::{StorageMap, StorageU64, StorageU256},
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
    /// Error returned when type failed to decode
    #[derive(Debug)]
    #[allow(missing_docs)]
    error FailedToDecode();

    /// Error returned when remote contract call fails
    #[derive(Debug)]
    #[allow(missing_docs)]
    error RemoteContractCallFailed();

    /// Error returned when value fails to encode
    #[derive(Debug)]
    #[allow(missing_docs)]
    error FailedToEncodeValue();
}

/// [VestingWallet] error
#[derive(SolidityError, Debug)]
pub enum Error {
    /// Error returned when decoding value returned after remote contract call
    FailedToDecodeValue(FailedToDecode),
    /// Error returned when contract call fails, i.e reading contract value, sending Erc20 tokens
    RemoteContractCallFailed(RemoteContractCallFailed),
    /// Error returned when encoding value fails
    FailedToEncode(FailedToEncodeValue),
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
    ownable: Ownable,
}

/// Trait `IVesting` defines all necessary vesting functionality per
/// `OpenZeppelin` solidity implementation
#[interface_id]
pub trait IVesting {
    /// Error type returned in methods
    type Error: Into<alloc::vec::Vec<u8>>;

    /// The contract should be able to receive ether token
    ///
    /// **Arguments**
    ///
    ///  * `&mut self` - allowing mutating contract account balance state
    fn receive_eth(&mut self);

    /// Beneficiary will call this function to receive vested ether tokens
    ///
    /// **Arguments**
    ///
    ///  * `&mut self` - allowing mutating contract and beneficiary account balance state
    ///
    /// **Event**
    ///
    /// Emits [EtherReleased] event
    ///
    /// **Error**
    ///
    /// Returns encoded error type defined on `transfer_eth` function
    fn release_eth(&mut self) -> Result<(), Vec<u8>>;

    /// Beneficiary will call this function to receive vested ERC-20 tokens
    ///
    /// **Arguments**
    ///
    ///  * `&mut self` - allowing mutating contract and beneficiary account balance state
    ///  * `token` - specifying which ERC-20 token address to release
    ///
    /// **Event**
    ///
    /// Emits [ERC20Released] event
    ///
    /// **Error**
    ///
    /// Returns [Error::RemoteContractCallFailed] if it fails to send ERC20 token to beneficiary
    fn release_erc20(&mut self, token: Address) -> Result<(), Self::Error>;

    /// Calculates the amount of ether that has already vested.
    /// Default implementation is a linear vesting curve.
    ///
    /// **Arguments**
    ///
    /// * `timestamp` - block timestamp
    fn vested_eth_amount(&self, timestamp: u64) -> U256;

    /// Calculates the amount of ERC-20 that has already vested.
    /// Default implementation is a linear vesting curve.
    ///
    /// **Arguments**
    ///
    /// * `token` - Erc20 contract address
    /// * `timestamp` -  block timestamp
    ///
    /// **Error**
    ///
    /// returns [FailedToDecode] or [RemoteContractCallFailed]
    fn vested_erc20_amount(
        &mut self,
        token: Address,
        timestamp: u64,
    ) -> Result<U256, Self::Error>;

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
    /// **Argument**
    ///
    /// * `token` - ERC-20 token address
    fn released_erc20(&self, token: Address) -> U256;
}

unsafe impl TopLevelStorage for VestingWallet {}

#[public]
impl IVesting for VestingWallet {
    type Error = Error;
    #[payable]
    fn receive_eth(&mut self) {}

    fn release_eth(&mut self) -> Result<(), Vec<u8>> {
        let amount = self.releasable_eth();
        let current_eth_released = self.released_eth() + amount;
        self.eth_released.set(current_eth_released);

        let owner = self.ownable.owner();
        // SAFETY: transfer cannot fail;
        transfer_eth(owner, amount)?;
        evm::log(EtherReleased { beneficiary: owner, value: amount });
        Ok(())
    }

    fn release_erc20(&mut self, token: Address) -> Result<(), Self::Error> {
        let amount = self.releasable_erc20(token)?;
        let current_erc20_released = self.released_erc20(token) + amount;
        self.erc20_released.insert(token, current_erc20_released);

        // remote Erc20 contract transfer call
        let call_function =
            function_selector!("transfer", Address, U256).to_vec();
        // SAFETY: cannot panic as address are 20 bytes in length;
        let beneficiary: [u8; 20] = self.ownable.owner().try_into().unwrap();

        let call_data = ethabi::encode(&[
            Token::Bytes(call_function),
            Token::Address(beneficiary.into()),
            Token::Bytes(amount.to_be_bytes_vec()),
        ]);

        let _result =
            call(Call::new_in(self), token, &call_data).map_err(|_| {
                Error::RemoteContractCallFailed(RemoteContractCallFailed {})
            })?;
        Ok(())
    }

    fn vested_eth_amount(&self, timestamp: u64) -> U256 {
        let balance = contract::balance();
        // SAFETY: cannot panic, as timestamp is always u64;
        let timestamp = U64::try_from(timestamp).unwrap();
        self.vesting_schedule(balance + self.released_eth(), timestamp)
    }

    fn vested_erc20_amount(
        &mut self,
        token: Address,
        timestamp: u64,
    ) -> Result<U256, Self::Error> {
        // remote ERC20 contract balance_of call
        let balance: U256 = {
            let call_function =
                function_selector!("balanceOf", Address).to_vec();
            // SAFETY: cannot panic as address are 20 bytes in length;
            let vesting_address: [u8; 20] =
                contract::address().to_vec().try_into().unwrap();

            let call_data = ethabi::encode(&[
                Token::Bytes(call_function),
                Token::Address(vesting_address.into()),
            ]);

            let encoded_erc20_balance = static_call(
                Call::new_in(self),
                token,
                &call_data,
            )
            .map_err(|_| {
                Error::RemoteContractCallFailed(RemoteContractCallFailed {})
            })?;

            Uint::<256>::abi_decode(&encoded_erc20_balance, true)
                .map_err(|_| Error::FailedToDecodeValue(FailedToDecode {}))?
        };

        // SAFETY: cannot panic, as timestamp is always u64;
        let timestamp = U64::try_from(timestamp).unwrap();
        Ok(self
            .vesting_schedule(balance + self.released_erc20(token), timestamp))
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

    fn released_eth(&self) -> U256 {
        *self.eth_released
    }

    fn released_erc20(&self, token: Address) -> U256 {
        self.erc20_released.get(token)
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
            // calculate the elapsed time as a fraction of duration and multiplying it by the allocated amount.
            (total_alloc * (timestamp - self.start()).to::<U256>())
                / self.duration()
        }
    }
    /// Getter for the amount of releasable eth.
    fn releasable_eth(&self) -> U256 {
        let timestamp = block::timestamp();
        self.vested_eth_amount(timestamp) - self.released_eth()
    }

    /// Getter for the amount of releasable ERC-20 token.
    ///
    /// **Arguments**
    ///
    ///  * `token` - specifying ERC-20 token contract address
    fn releasable_erc20(&mut self, token: Address) -> Result<U256, Error> {
        let timestamp = block::timestamp();
        let vested_erc20 = self.vested_erc20_amount(token, timestamp)?;
        Ok(vested_erc20 - self.released_erc20(token))
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::block;
    use crate::finance::vesting_wallet::{IVesting, VestingWallet};
    use crate::token::erc20::{Erc20, IErc20};
    use alloy_primitives::{address, uint, U256, U64};
    use motsu::prelude::*;
    use stylus_sdk::{call::transfer_eth, contract};

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
        // error returned: dyld[74269]: missing symbol called when calling contract::balance();
        // no function in motsu::shim
        // assert_eq!(amount,contrac&self,call: Call<()>, account: Address// but it will be covered in e2e tests
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
        // testing one year linear vesting, should return fraction of total allocation
        // the schedule tested quarterly

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
        // lock funds until timestamp >= start, and release all funds without vesting
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
        let amount = uint!(100_000_U256) * uint!(10_U256).pow(uint!(18_U256));

        contract.start.set(U64::try_from(start).unwrap());
        contract.duration.set(duration);

        contract.eth_released.set(uint!(0_U256));
        let expected_q1 =
            uint!(25_000_U256) * uint!(10_U256).pow(uint!(18_U256));

        // assert_eq!(contract.vested_eth_amount(start + 5_929_200_u64),expected_q1);
        // cannot compile due to contract::balance() not available in the SHIM
        // this will be tested in e2e tests
    }
}
