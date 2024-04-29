//! Implementation of the ERC-20 token standard.
//!
//! We have followed general ``OpenZeppelin`` Contracts guidelines: functions
//! revert instead of returning `false` on failure. This behavior is
//! nonetheless conventional and does not conflict with the expectations of
//! ERC-20 applications.
use alloy_primitives::{Address, U256};
use alloy_sol_types::sol;
use stylus_proc::SolidityError;
use stylus_sdk::{
    evm, msg,
    prelude::{external, sol_storage},
};

pub mod extensions;

/// This macro provides an implementation of the ERC-20 token.
///
/// It adds all the functions from the `IERC20` trait and expects the token
/// to contain `erc20` attribute that implements `IERC20` trait too.
///
/// Used to export interface for Stylus smart contract with a single
/// `#[external]` macro.
#[allow(clippy::module_name_repetitions)]
#[macro_export]
macro_rules! erc20_impl {
    () => {
        /// Returns the number of tokens in existence.
        ///
        /// See [`IERC20::total_supply`].
        pub(crate) fn total_supply(&self) -> alloy_primitives::U256 {
            self.erc20.total_supply()
        }

        /// Returns the number of tokens owned by `account`.
        ///
        /// See [`IERC20::balance_of`].
        pub(crate) fn balance_of(
            &self,
            account: alloy_primitives::Address,
        ) -> alloy_primitives::U256 {
            self.erc20.balance_of(account)
        }

        /// Moves a `value` amount of tokens from the caller's account to `to`.
        ///
        /// Returns a boolean value indicating whether the operation succeeded.
        ///
        /// See [`IERC20::transfer`].
        pub(crate) fn transfer(
            &mut self,
            to: alloy_primitives::Address,
            value: alloy_primitives::U256,
        ) -> Result<bool, alloc::vec::Vec<u8>> {
            self.erc20.transfer(to, value).map_err(|e| e.into())
        }

        /// Returns the remaining number of tokens that `spender` will be
        /// allowed to spend on behalf of `owner` through `transfer_from`. This
        /// is zero by default.
        ///
        /// See [`IERC20::allowance`].
        pub(crate) fn allowance(
            &self,
            owner: alloy_primitives::Address,
            spender: alloy_primitives::Address,
        ) -> alloy_primitives::U256 {
            self.erc20.allowance(owner, spender)
        }

        /// Sets a `value` number of tokens as the allowance of `spender` over
        /// the caller's tokens.
        ///
        /// Returns a boolean value indicating whether the operation succeeded.
        ///
        /// See [`IERC20::approve`].
        pub(crate) fn approve(
            &mut self,
            spender: alloy_primitives::Address,
            value: alloy_primitives::U256,
        ) -> Result<bool, alloc::vec::Vec<u8>> {
            self.erc20.approve(spender, value).map_err(|e| e.into())
        }

        /// Moves a `value` number of tokens from `from` to `to` using the
        /// allowance mechanism. `value` is then deducted from the caller's
        /// allowance.
        ///
        /// Returns a boolean value indicating whether the operation succeeded.
        ///
        /// See [`IERC20::transfer_from`].
        pub(crate) fn transfer_from(
            &mut self,
            from: alloy_primitives::Address,
            to: alloy_primitives::Address,
            value: alloy_primitives::U256,
        ) -> Result<bool, alloc::vec::Vec<u8>> {
            self.erc20.transfer_from(from, to, value).map_err(|e| e.into())
        }
    };
}

sol_storage! {
    /// State of an ERC20 token.
    pub struct ERC20 {
        /// Maps users to balances.
        mapping(address => uint256) _balances;
        /// Maps users to a mapping of each spender's allowance.
        mapping(address => mapping(address => uint256)) _allowances;
        /// The total supply of the token.
        uint256 _total_supply;
    }
}

sol! {
    /// Emitted when `value` tokens are moved from one account (`from`) to
    /// another (`to`).
    ///
    /// Note that `value` may be zero.
    event Transfer(address indexed from, address indexed to, uint256 value);
    /// Emitted when the allowance of a `spender` for an `owner` is set by a
    /// call to `approve`. `value` is the new allowance.
    event Approval(address indexed owner, address indexed spender, uint256 value);
}

sol! {
    /// Indicates an error related to the current `balance` of `sender`. Used
    /// in transfers.
    ///
    /// * `sender` - Address whose tokens are being transferred.
    /// * `balance` - Current balance for the interacting account.
    /// * `needed` - Minimum amount required to perform a transfer.
    #[derive(Debug)]
    error ERC20InsufficientBalance(address sender, uint256 balance, uint256 needed);
    /// Indicates a failure with the token `sender`. Used in transfers.
    ///
    /// * `sender` - Address whose tokens are being transferred.
    #[derive(Debug)]
    error ERC20InvalidSender(address sender);
    /// Indicates a failure with the token `receiver`. Used in transfers.
    ///
    /// * `receiver` - Address to which the tokens are being transferred.
    #[derive(Debug)]
    error ERC20InvalidReceiver(address receiver);
    /// Indicates a failure with the `spender`’s `allowance`. Used in
    /// transfers.
    ///
    /// * `spender` - Address that may be allowed to operate on tokens without
    /// being their owner.
    /// * `allowance` - Amount of tokens a `spender` is allowed to operate
    /// with.
    /// * `needed` - Minimum amount required to perform a transfer.
    #[derive(Debug)]
    error ERC20InsufficientAllowance(address spender, uint256 allowance, uint256 needed);
    /// Indicates a failure with the `spender` to be approved. Used in
    /// approvals.
    ///
    /// * `spender` - Address that may be allowed to operate on tokens without
    /// being their owner.
    #[derive(Debug)]
    error ERC20InvalidSpender(address spender);

}

/// An ERC-20 error defined as described in [ERC-6093].
///
/// [ERC-6093]: https://eips.ethereum.org/EIPS/eip-6093
#[derive(SolidityError, Debug)]
pub enum Error {
    /// Indicates an error related to the current balance of `sender`. Used in
    /// transfers.
    InsufficientBalance(ERC20InsufficientBalance),
    /// Indicates a failure with the token `sender`. Used in transfers.
    InvalidSender(ERC20InvalidSender),
    /// Indicates a failure with the token `receiver`. Used in transfers.
    InvalidReceiver(ERC20InvalidReceiver),
    /// Indicates a failure with the `spender`’s `allowance`. Used in
    /// transfers.
    InsufficientAllowance(ERC20InsufficientAllowance),
    /// Indicates a failure with the `spender` to be approved. Used in
    /// approvals.
    InvalidSpender(ERC20InvalidSpender),
    /// TODO!!!
    ERC20PausableError(crate::utils::pausable::EnforcedPause),
    /// Indicates a failure when total supply cap has been exceeded.
    ERC20ExceededCap(crate::utils::capped::ExceededCap),
}

/// Interface of storage management for ERC20 token.
pub trait IERC20Storage {
    /// Returns the number of tokens in existence.
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    fn _get_total_supply(&self) -> U256;

    /// Sets the number of tokens in existence.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `total_supply` - Number of tokens in existence.
    fn _set_total_supply(&mut self, total_supply: U256);

    /// Returns the number of tokens owned by `account`.
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    /// * `account` - Account to get balance from.
    fn _get_balance(&self, account: Address) -> U256;

    /// Sets a `value` number of tokens owned by `account`.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `owner` - Account that owns the tokens.
    /// * `value` - Number of tokens that the account has.
    fn _set_balance(&mut self, account: Address, balance: U256);

    /// Returns the remaining number of tokens that `spender` will be allowed
    /// to spend on behalf of `owner`. This is zero by default.
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    /// * `owner` - Account that owns the tokens.
    /// * `spender` - Account that will spend the tokens.
    fn _get_allowance(&self, owner: Address, spender: Address) -> U256;

    /// Sets a `value` number of tokens as the allowance of `spender` over the
    /// caller's tokens.
    ///
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `owner` - Account that owns the tokens.
    /// * `spender` - Account that will spend the tokens.
    fn _set_allowance(
        &mut self,
        owner: Address,
        spender: Address,
        allowance: U256,
    );
}

/// Implementation of storage management [`IERC20Storage`] for ERC20 token.
impl IERC20Storage for ERC20 {
    fn _get_total_supply(&self) -> U256 {
        self._total_supply.get()
    }

    fn _set_total_supply(&mut self, total_supply: U256) {
        self._total_supply.set(total_supply);
    }

    fn _get_balance(&self, account: Address) -> U256 {
        self._balances.get(account)
    }

    fn _set_balance(&mut self, account: Address, balance: U256) {
        self._balances.setter(account).set(balance);
    }

    fn _get_allowance(&self, owner: Address, spender: Address) -> U256 {
        self._allowances.get(owner).get(spender)
    }

    fn _set_allowance(
        &mut self,
        owner: Address,
        spender: Address,
        allowance: U256,
    ) {
        self._allowances.setter(owner).insert(spender, allowance);
    }
}

/// Interface of the ERC20 internal (private) functions.
pub trait IERC20Virtual: IERC20Storage {
    /// Internal implementation of transferring tokens between two accounts.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `from` - Account to transfer tokens from.
    /// * `to` - Account to transfer tokens to.
    /// * `value` - The number of tokens to transfer.
    ///
    /// # Errors
    ///
    /// * If the `from` address is `Address::ZERO`, then the error
    /// [`Error::InvalidSender`] is returned.
    /// * If the `to` address is `Address::ZERO`, then the error
    /// [`Error::InvalidReceiver`] is returned.
    /// If the `from` address doesn't have enough tokens, then the error
    /// [`Error::InsufficientBalance`] is returned.
    ///
    /// # Events
    ///
    /// Emits a [`Transfer`] event.
    fn _transfer(
        &mut self,
        from: Address,
        to: Address,
        value: U256,
    ) -> Result<(), Error> {
        if from.is_zero() {
            return Err(Error::InvalidSender(ERC20InvalidSender {
                sender: Address::ZERO,
            }));
        }
        if to.is_zero() {
            return Err(Error::InvalidReceiver(ERC20InvalidReceiver {
                receiver: Address::ZERO,
            }));
        }

        self._update(from, to, value)?;

        Ok(())
    }

    /// Transfers a `value` amount of tokens from `from` to `to`,
    /// or alternatively mints (or burns)
    /// if `from` (or `to`) is the zero address.
    ///
    /// All customizations to transfers, mints, and burns
    /// should be done by using this function.
    ///
    /// # Arguments
    ///
    /// * `from` - Owner's address.
    /// * `to` - Recipient's address.
    /// * `value` - Amount to be transferred.
    ///
    /// # Panics
    ///
    /// If `_total_supply` exceeds `U256::MAX`.
    /// It may happen during `mint` operation.
    ///
    /// # Errors
    ///
    /// If the `from` address doesn't have enough tokens, then the error
    /// [`Error::InsufficientBalance`] is returned.
    ///
    /// # Events
    ///
    /// Emits a [`Transfer`] event.
    fn _update(
        &mut self,
        from: Address,
        to: Address,
        value: U256,
    ) -> Result<(), Error> {
        if from.is_zero() {
            // Mint operation. Overflow check required: the rest of the code
            // assumes that `_total_supply` never overflows.
            let total_supply = self
                ._get_total_supply()
                .checked_add(value)
                .expect("Should not exceed `U256::MAX` for `_total_supply`");
            self._set_total_supply(total_supply);
        } else {
            let from_balance = self._get_balance(from);
            if from_balance < value {
                return Err(Error::InsufficientBalance(
                    ERC20InsufficientBalance {
                        sender: from,
                        balance: from_balance,
                        needed: value,
                    },
                ));
            }
            // Overflow not possible:
            // value <= from_balance <= _total_supply.
            self._set_balance(from, from_balance - value);
        }

        if to.is_zero() {
            let total_supply = self._get_total_supply();
            // Overflow not possible:
            // value <= _total_supply or value <= from_balance <= _total_supply.
            self._set_total_supply(total_supply - value);
        } else {
            let balance_to = self._get_balance(to);
            // Overflow not possible:
            // balance + value is at most total_supply, which fits into a U256.
            self._set_balance(to, balance_to + value);
        }

        evm::log(Transfer { from, to, value });

        Ok(())
    }

    /// Destroys a `value` amount of tokens from `account`,
    /// lowering the total supply.
    ///
    /// Relies on the `update` mechanism.
    ///
    /// # Arguments
    ///
    /// * `account` - Owner's address.
    /// * `value` - Amount to be burnt.
    ///
    /// # Errors
    ///
    /// * If the `from` address is `Address::ZERO`, then the error
    /// [`Error::InvalidSender`] is returned.
    /// If the `from` address doesn't have enough tokens, then the error
    /// [`Error::InsufficientBalance`] is returned.
    ///
    /// # Events
    ///
    /// Emits a [`Transfer`] event.
    fn _burn(&mut self, account: Address, value: U256) -> Result<(), Error> {
        if account == Address::ZERO {
            return Err(Error::InvalidSender(ERC20InvalidSender {
                sender: Address::ZERO,
            }));
        }
        self._update(account, Address::ZERO, value)
    }

    /// Updates `owner`'s allowance for `spender` based on spent `value`.
    ///
    /// Does not update the allowance value in the case of infinite allowance.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `owner` - Account to transfer tokens from.
    /// * `to` - Account to transfer tokens to.
    /// * `value` - The number of tokens to transfer.
    ///
    /// # Errors
    ///
    /// If not enough allowance is available, then the error
    /// [`Error::InsufficientAllowance`] is returned.
    fn _spend_allowance(
        &mut self,
        owner: Address,
        spender: Address,
        value: U256,
    ) -> Result<(), Error> {
        let current_allowance = self._get_allowance(owner, spender);
        if current_allowance != U256::MAX {
            if current_allowance < value {
                return Err(Error::InsufficientAllowance(
                    ERC20InsufficientAllowance {
                        spender,
                        allowance: current_allowance,
                        needed: value,
                    },
                ));
            }

            self._set_allowance(owner, spender, current_allowance - value);
        }

        Ok(())
    }
}

/// Interface of the ERC20 standard as defined in the EIP.
pub trait IERC20: IERC20Virtual {
    /// Returns the number of tokens in existence.
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    fn total_supply(&self) -> U256 {
        self._get_total_supply()
    }

    /// Returns the number of tokens owned by `account`.
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    /// * `account` - Account to get balance from.
    fn balance_of(&self, account: Address) -> U256 {
        self._get_balance(account)
    }

    /// Moves a `value` amount of tokens from the caller's account to `to`.
    ///
    /// Returns a boolean value indicating whether the operation succeeded.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `to` - Account to transfer tokens to.
    /// * `value` - Number of tokens to transfer.
    ///
    /// # Errors
    ///
    /// * If the `to` address is `Address::ZERO`, then the error
    /// [`Error::InvalidReceiver`] is returned.
    /// * If the caller doesn't have a balance of at least `value`, then the
    /// error [`Error::InsufficientBalance`] is returned.
    ///
    /// # Events
    ///
    /// Emits a [`Transfer`] event.
    fn transfer(&mut self, to: Address, value: U256) -> Result<bool, Error> {
        let from = msg::sender();
        self._transfer(from, to, value)?;
        Ok(true)
    }

    /// Returns the remaining number of tokens that `spender` will be allowed
    /// to spend on behalf of `owner` through `transfer_from`. This is zero by
    /// default.
    ///
    /// This value changes when `approve` or `transfer_from` are called.
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    /// * `owner` - Account that owns the tokens.
    /// * `spender` - Account that will spend the tokens.
    fn allowance(&self, owner: Address, spender: Address) -> U256 {
        self._get_allowance(owner, spender)
    }

    /// Sets a `value` number of tokens as the allowance of `spender` over the
    /// caller's tokens.
    ///
    /// Returns a boolean value indicating whether the operation succeeded.
    ///
    /// WARNING: Beware that changing an allowance with this method brings the
    /// risk that someone may use both the old and the new allowance by
    /// unfortunate transaction ordering. One possible solution to mitigate
    /// this race condition is to first reduce the spender's allowance to 0 and
    /// set the desired value afterwards:
    /// <https://github.com/ethereum/EIPs/issues/20#issuecomment-263524729>
    ///
    /// # Arguments
    ///
    /// * `&mutself` - Write access to the contract's state.
    /// * `owner` - Account that owns the tokens.
    /// * `spender` - Account that will spend the tokens.
    ///
    /// # Errors
    ///
    /// If the `spender` address is `Address::ZERO`, then the error
    /// [`Error::InvalidSpender`] is returned.
    ///
    /// # Events
    ///
    /// Emits an [`Approval`] event.
    fn approve(
        &mut self,
        spender: Address,
        value: U256,
    ) -> Result<bool, Error> {
        let owner = msg::sender();
        if spender.is_zero() {
            return Err(Error::InvalidSpender(ERC20InvalidSpender {
                spender: Address::ZERO,
            }));
        }

        self._set_allowance(owner, spender, value);
        evm::log(Approval { owner, spender, value });
        Ok(true)
    }

    /// Moves a `value` number of tokens from `from` to `to` using the
    /// allowance mechanism. `value` is then deducted from the caller's
    /// allowance.
    ///
    /// Returns a boolean value indicating whether the operation succeeded.
    ///
    /// NOTE: If `value` is the maximum `uint256`, the allowance is not updated
    /// on `transferFrom`. This is semantically equivalent to an infinite
    /// approval.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `from` - Account to transfer tokens from.
    /// * `to` - Account to transfer tokens to.
    /// * `value` - Number of tokens to transfer.
    ///
    /// # Errors
    ///
    /// * If the `from` address is `Address::ZERO`, then the error
    /// [`Error::InvalidSender`] is returned.
    /// * If the `to` address is `Address::ZERO`, then the error
    /// [`Error::InvalidReceiver`] is returned.
    /// * If not enough allowance is available, then the error
    /// [`Error::InsufficientAllowance`] is returned.
    ///
    /// # Events
    ///
    /// Emits a [`Transfer`] event.
    fn transfer_from(
        &mut self,
        from: Address,
        to: Address,
        value: U256,
    ) -> Result<bool, Error> {
        let spender = msg::sender();
        self._spend_allowance(from, spender, value)?;
        self._transfer(from, to, value)?;
        Ok(true)
    }
}

/// Default implementation of `IERC20Virtual` trait for `ERC20`.
impl IERC20Virtual for ERC20 {}

/// Default implementation of `IERC20` trait for `ERC20`.
#[external]
impl IERC20 for ERC20 {}

#[cfg(test)]
mod tests {
    use alloy_primitives::{address, Address, U256};
    use stylus_sdk::{
        msg,
        storage::{StorageMap, StorageType, StorageU256},
    };

    use crate::erc20::{Error, IERC20Storage, IERC20Virtual, ERC20, IERC20};

    impl Default for ERC20 {
        fn default() -> Self {
            let root = U256::ZERO;
            ERC20 {
                _balances: unsafe { StorageMap::new(root, 0) },
                _allowances: unsafe {
                    StorageMap::new(root + U256::from(32), 0)
                },
                _total_supply: unsafe {
                    StorageU256::new(root + U256::from(64), 0)
                },
            }
        }
    }

    #[grip::test]
    fn reads_balance(contract: ERC20) {
        let balance = contract.balance_of(Address::ZERO);
        assert_eq!(U256::ZERO, balance);

        let owner = msg::sender();
        let one = U256::from(1);
        contract._set_balance(owner, one);
        let balance = contract.balance_of(owner);
        assert_eq!(one, balance);
    }

    #[grip::test]
    fn update_mint(contract: ERC20) {
        let alice = address!("A11CEacF9aa32246d767FCCD72e02d6bCbcC375d");
        let one = U256::from(1);

        // Store initial balance & supply
        let initial_balance = contract.balance_of(alice);
        let initial_supply = contract.total_supply();

        // Mint action should work
        let result = contract._update(Address::ZERO, alice, one);
        assert!(result.is_ok());

        // Check updated balance & supply
        assert_eq!(initial_balance + one, contract.balance_of(alice));
        assert_eq!(initial_supply + one, contract.total_supply());
    }

    #[grip::test]
    #[should_panic(
        expected = "Should not exceed `U256::MAX` for `_total_supply`"
    )]
    fn update_mint_errors_arithmetic_overflow(contract: ERC20) {
        let alice = address!("A11CEacF9aa32246d767FCCD72e02d6bCbcC375d");
        let one = U256::from(1);
        assert_eq!(U256::ZERO, contract.balance_of(alice));
        assert_eq!(U256::ZERO, contract.total_supply());

        // Initialize state for the test case -- Alice's balance as U256::MAX
        contract
            ._update(Address::ZERO, alice, U256::MAX)
            .expect("ERC20::_update should work");
        // Mint action should NOT work -- overflow on `_total_supply`.
        let _result = contract._update(Address::ZERO, alice, one);
    }

    #[grip::test]
    fn update_burn(contract: ERC20) {
        let alice = address!("A11CEacF9aa32246d767FCCD72e02d6bCbcC375d");
        let one = U256::from(1);
        let two = U256::from(2);

        // Initialize state for the test case -- Alice's balance as `two`
        contract
            ._update(Address::ZERO, alice, two)
            .expect("ERC20::_update should work");

        // Store initial balance & supply
        let initial_balance = contract.balance_of(alice);
        let initial_supply = contract.total_supply();

        // Burn action should work
        let result = contract._update(alice, Address::ZERO, one);
        assert!(result.is_ok());

        // Check updated balance & supply
        assert_eq!(initial_balance - one, contract.balance_of(alice));
        assert_eq!(initial_supply - one, contract.total_supply());
    }

    #[grip::test]
    fn update_burn_errors_insufficient_balance(contract: ERC20) {
        let alice = address!("A11CEacF9aa32246d767FCCD72e02d6bCbcC375d");
        let one = U256::from(1);
        let two = U256::from(2);

        // Initialize state for the test case -- Alice's balance as `one`
        contract
            ._update(Address::ZERO, alice, one)
            .expect("ERC20::_update should work");

        // Store initial balance & supply
        let initial_balance = contract.balance_of(alice);
        let initial_supply = contract.total_supply();

        // Burn action should NOT work -- `InsufficientBalance`
        let result = contract._update(alice, Address::ZERO, two);
        assert!(matches!(result, Err(Error::InsufficientBalance(_))));

        // Check proper state (before revert)
        assert_eq!(initial_balance, contract.balance_of(alice));
        assert_eq!(initial_supply, contract.total_supply());
    }

    #[grip::test]
    fn update_transfer(contract: ERC20) {
        let alice = address!("A11CEacF9aa32246d767FCCD72e02d6bCbcC375d");
        let bob = address!("B0B0cB49ec2e96DF5F5fFB081acaE66A2cBBc2e2");
        let one = U256::from(1);

        // Initialize state for the test case -- Alice's & Bob's balance as
        // `one`
        contract
            ._update(Address::ZERO, alice, one)
            .expect("ERC20::_update should work");
        contract
            ._update(Address::ZERO, bob, one)
            .expect("ERC20::_update should work");

        // Store initial balance & supply
        let initial_alice_balance = contract.balance_of(alice);
        let initial_bob_balance = contract.balance_of(bob);
        let initial_supply = contract.total_supply();

        // Transfer action should work
        let result = contract._update(alice, bob, one);
        assert!(result.is_ok());

        // Check updated balance & supply
        assert_eq!(initial_alice_balance - one, contract.balance_of(alice));
        assert_eq!(initial_bob_balance + one, contract.balance_of(bob));
        assert_eq!(initial_supply, contract.total_supply());
    }

    #[grip::test]
    fn update_transfer_errors_insufficient_balance(contract: ERC20) {
        let alice = address!("A11CEacF9aa32246d767FCCD72e02d6bCbcC375d");
        let bob = address!("B0B0cB49ec2e96DF5F5fFB081acaE66A2cBBc2e2");
        let one = U256::from(1);

        // Initialize state for the test case -- Alice's & Bob's balance as
        // `one`
        contract
            ._update(Address::ZERO, alice, one)
            .expect("ERC20::_update should work");
        contract
            ._update(Address::ZERO, bob, one)
            .expect("ERC20::_update should work");

        // Store initial balance & supply
        let initial_alice_balance = contract.balance_of(alice);
        let initial_bob_balance = contract.balance_of(bob);
        let initial_supply = contract.total_supply();

        // Transfer action should NOT work -- `InsufficientBalance`
        let result = contract._update(alice, bob, one + one);
        assert!(matches!(result, Err(Error::InsufficientBalance(_))));

        // Check proper state (before revert)
        assert_eq!(initial_alice_balance, contract.balance_of(alice));
        assert_eq!(initial_bob_balance, contract.balance_of(bob));
        assert_eq!(initial_supply, contract.total_supply());
    }

    #[grip::test]
    fn transfers(contract: ERC20) {
        let alice = address!("A11CEacF9aa32246d767FCCD72e02d6bCbcC375d");
        let bob = address!("B0B0cB49ec2e96DF5F5fFB081acaE66A2cBBc2e2");

        // Alice approves `msg::sender`.
        let one = U256::from(1);
        contract._set_allowance(alice, msg::sender(), one);

        // Mint some tokens for Alice.
        let two = U256::from(2);
        contract._update(Address::ZERO, alice, two).unwrap();
        assert_eq!(two, contract.balance_of(alice));

        contract.transfer_from(alice, bob, one).unwrap();

        assert_eq!(one, contract.balance_of(alice));
        assert_eq!(one, contract.balance_of(bob));
    }

    #[grip::test]
    fn transfers_from(contract: ERC20) {
        let alice = address!("A11CEacF9aa32246d767FCCD72e02d6bCbcC375d");
        let bob = address!("B0B0cB49ec2e96DF5F5fFB081acaE66A2cBBc2e2");
        let sender = msg::sender();

        // Alice approves `msg::sender`.
        let one = U256::from(1);
        contract._set_allowance(alice, sender, one);

        // Mint some tokens for Alice.
        let two = U256::from(2);
        contract._update(Address::ZERO, alice, two).unwrap();
        assert_eq!(two, contract.balance_of(alice));

        contract.transfer_from(alice, bob, one).unwrap();

        assert_eq!(one, contract.balance_of(alice));
        assert_eq!(one, contract.balance_of(bob));
        assert_eq!(U256::ZERO, contract.allowance(alice, sender));
    }

    #[grip::test]
    fn transfer_from_errors_when_insufficient_balance(contract: ERC20) {
        let alice = address!("A11CEacF9aa32246d767FCCD72e02d6bCbcC375d");
        let bob = address!("B0B0cB49ec2e96DF5F5fFB081acaE66A2cBBc2e2");

        // Alice approves `msg::sender`.
        let one = U256::from(1);
        contract._set_allowance(alice, msg::sender(), one);
        assert_eq!(U256::ZERO, contract.balance_of(alice));

        let one = U256::from(1);
        let result = contract.transfer_from(alice, bob, one);
        assert!(matches!(result, Err(Error::InsufficientBalance(_))));
    }

    #[grip::test]
    fn transfer_from_errors_when_invalid_sender(contract: ERC20) {
        let alice = address!("A11CEacF9aa32246d767FCCD72e02d6bCbcC375d");
        let one = U256::from(1);
        contract._set_allowance(Address::ZERO, msg::sender(), one);
        let result = contract.transfer_from(Address::ZERO, alice, one);
        assert!(matches!(result, Err(Error::InvalidSender(_))));
    }

    #[grip::test]
    fn transfer_from_errors_when_invalid_receiver(contract: ERC20) {
        let alice = address!("A11CEacF9aa32246d767FCCD72e02d6bCbcC375d");
        let one = U256::from(1);
        contract._set_allowance(alice, msg::sender(), one);
        let result = contract.transfer_from(alice, Address::ZERO, one);
        assert!(matches!(result, Err(Error::InvalidReceiver(_))));
    }

    #[grip::test]
    fn transfer_from_errors_when_insufficient_allowance(contract: ERC20) {
        let alice = address!("A11CEacF9aa32246d767FCCD72e02d6bCbcC375d");
        let bob = address!("B0B0cB49ec2e96DF5F5fFB081acaE66A2cBBc2e2");

        // Mint some tokens for Alice.
        let one = U256::from(1);
        contract._update(Address::ZERO, alice, one).unwrap();
        assert_eq!(one, contract.balance_of(alice));

        let result = contract.transfer_from(alice, bob, one);
        assert!(matches!(result, Err(Error::InsufficientAllowance(_))));
    }

    #[grip::test]
    fn reads_allowance(contract: ERC20) {
        let owner = msg::sender();
        let alice = address!("A11CEacF9aa32246d767FCCD72e02d6bCbcC375d");

        let allowance = contract.allowance(owner, alice);
        assert_eq!(U256::ZERO, allowance);

        let one = U256::from(1);
        contract._set_allowance(owner, alice, one);
        let allowance = contract.allowance(owner, alice);
        assert_eq!(one, allowance);
    }

    #[grip::test]
    fn approves(contract: ERC20) {
        let alice = address!("A11CEacF9aa32246d767FCCD72e02d6bCbcC375d");

        // `msg::sender` approves Alice.
        let one = U256::from(1);
        contract.approve(alice, one).unwrap();
        assert_eq!(one, contract._get_allowance(msg::sender(), alice));
    }

    #[grip::test]
    fn approve_errors_when_invalid_spender(contract: ERC20) {
        // `msg::sender` approves `Address::ZERO`.
        let one = U256::from(1);
        let result = contract.approve(Address::ZERO, one);
        assert!(matches!(result, Err(Error::InvalidSpender(_))));
    }
}
