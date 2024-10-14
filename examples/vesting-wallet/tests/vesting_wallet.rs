#![cfg(feature = "e2e")]

mod abi;
use abi::VestingWallet;
use alloy::{
    primitives::{uint, Address, U256},
    providers::{Provider, WalletProvider},
    rpc::types::TransactionRequest,
    sol,
};
use alloy_primitives::utils::parse_ether;
use e2e::{
    receipt, send, watch, Account, EventExt, Panic, PanicCode, ReceiptExt,
    Revert,
};
use eyre::Result;
use stylus_sdk::{alloy_sol_types::SolConstructor, block::timestamp};

use crate::VestingWalletExample::constructorCall;

sol!("src/constructor.sol");

fn vesting_constructor(
    alice: Address,
    start: u64,
    duration: u64,
) -> constructorCall {
    VestingWalletExample::constructorCall {
        beneficiary: alice,
        startTimestamp: start,
        durationSeconds: duration,
    }
}

// ============================================================================
// Integration Tests: VestingWallet
// ============================================================================

#[e2e::test]
async fn constructor(alice: Account) -> Result<()> {
    let start = timestamp();
    let duration = 23_716_800u64;

    let contract_addr = alice
        .as_deployer()
        .with_constructor(vesting_constructor(alice.address(), start, duration))
        .deploy()
        .await?
        .address()?;
    let contract = VestingWallet::new(contract_addr, &alice.wallet);

    let VestingWallet::startReturn { start } = contract.start().call().await?;
    let VestingWallet::durationReturn { duration } =
        contract.duration().call().await?;
    let VestingWallet::endReturn { end } = contract.end().call().await?;

    let VestingWallet::ownerReturn { owner } = contract.owner().call().await?;

    assert_eq!(end, start + duration);
    assert_eq!(owner, alice.address());
    Ok(())
}

#[e2e::test]
async fn initial_address_cannot_be_zero(alice: Account) -> Result<()> {
    let start = timestamp();
    let duration = 23_716_800u64;

    let err = alice
        .as_deployer()
        .with_constructor(vesting_constructor(Address::ZERO, start, duration))
        .deploy()
        .await
        .expect_err("should not deploy due to `OwnableInvalidOwner`");

    assert!(err.reverted_with(VestingWallet::OwnableInvalidOwner {
        owner: Address::ZERO
    }));
    Ok(())
}

#[e2e::test]
async fn contract_receiving_erc20_works() -> Result<()> {
    todo!()
}

// if duration is set to 0, then vesting contract will act as time lock contract
#[e2e::test]
async fn vesting_acts_like_lock_if_duration_zero(alice: Account) -> Result<()> {
    let timestamp = timestamp();
    let start = timestamp + 5_929_200_u64;
    let duration = 0u64;

    let contract_addr = alice
        .as_deployer()
        .with_constructor(vesting_constructor(alice.address(), start, duration))
        .deploy()
        .await?
        .address()?;
    let mut contract = VestingWallet::new(contract_addr, &alice.wallet);

    let VestingWallet::vestedEthAmountReturn { amount } =
        contract.vestedEthAmount(timestamp + 1_000_000).call().await?;
    assert_eq!(amount, uint!(0_U256));
    // when time elapses as it reaches start time the funds unlock
    let VestingWallet::vestedEthAmountReturn { amount } =
        contract.vestedEthAmount(start).call().await?;
    assert_eq!(amount, uint!(10_U256));

    Ok(())
}

// testing daily vesting
async fn vesting_schedule_works(alice: Account) -> Result<()> {
    let start = timestamp();
    let day = 86_400u64;
    let duration = day * 5; // 5 days vesting

    let contract_addr = alice
        .as_deployer()
        .with_constructor(vesting_constructor(alice.address(), start, duration))
        .deploy()
        .await?
        .address()?;
    let contract = VestingWallet::new(contract_addr, &alice.wallet);

    let VestingWallet::vestedEthAmountReturn { amount } =
        contract.vestedEthAmount(day).call().await?;
    assert_eq!(amount, uint!(2_U256));

    let VestingWallet::vestedEthAmountReturn { amount } =
        contract.vestedEthAmount(day * 2).call().await?;
    assert_eq!(amount, uint!(4_U256));

    let VestingWallet::vestedEthAmountReturn { amount } =
        contract.vestedEthAmount(day * 3).call().await?;
    assert_eq!(amount, uint!(6_U256));

    let VestingWallet::vestedEthAmountReturn { amount } =
        contract.vestedEthAmount(day * 4).call().await?;
    assert_eq!(amount, uint!(8_U256));

    let VestingWallet::vestedEthAmountReturn { amount } =
        contract.vestedEthAmount(day * 5).call().await?;
    assert_eq!(amount, uint!(10_U256));

    Ok(())
}

#[e2e::test]
async fn vesting_erc20_and_eth_works() -> Result<()> {
    todo!()
}

#[e2e::test]
async fn vesting_multiple_erc20_works() -> Result<()> {
    todo!()
}

// checking if adding eth to an existing vesting schedule will continue and use
// the updated eth amount
#[e2e::test]
async fn add_eth_to_existing_vesting_schedule_continues(
    alice: Account,
    bob: Account,
) -> Result<()> {
    let start = timestamp();
    let day = 86_400u64;
    let duration = day * 5; // 5 days vesting

    let contract_addr = alice
        .as_deployer()
        .with_constructor(vesting_constructor(alice.address(), start, duration))
        .deploy()
        .await?
        .address()?;
    let contract = VestingWallet::new(contract_addr, &alice.wallet);

    let contract_balance = alice.wallet.get_balance(contract_addr).await?;
    let expected = parse_ether("10")?;
    assert_eq!(contract_balance, expected);

    let VestingWallet::vestedEthAmountReturn { amount } =
        contract.vestedEthAmount(day).call().await?;
    assert_eq!(amount, uint!(2_U256));

    let VestingWallet::vestedEthAmountReturn { amount } =
        contract.vestedEthAmount(day * 2).call().await?;
    assert_eq!(amount, uint!(4_U256));

    // adding eth to the vesting contract
    let tx =
        TransactionRequest::default().to(contract_addr).value(uint!(5_U256));
    let _tx_receipt =
        alice.wallet.send_transaction(tx).await?.get_receipt().await?;

    let tx2 =
        TransactionRequest::default().to(contract_addr).value(uint!(7_U256));
    let _tx_receipt =
        bob.wallet.send_transaction(tx2).await?.get_receipt().await?;

    let contract_balance = alice.wallet.get_balance(contract_addr).await?;
    let expected = parse_ether("22")?;
    assert_eq!(contract_balance, expected);

    // vesting amount changes

    let VestingWallet::vestedEthAmountReturn { amount } =
        contract.vestedEthAmount(day * 3).call().await?;
    assert_eq!(amount, uint!(10_U256));

    let VestingWallet::vestedEthAmountReturn { amount } =
        contract.vestedEthAmount(day * 4).call().await?;
    assert_eq!(amount, uint!(16_U256));

    let VestingWallet::vestedEthAmountReturn { amount } =
        contract.vestedEthAmount(day * 5).call().await?;
    assert_eq!(amount, uint!(22_U256));

    Ok(())
}

// checking if adding Erc20 token to an existing vesting schedule will continue
// and use the updated Erc20 token amount
#[e2e::test]
async fn add_erc20_to_existing_vesting_schedule_continues() -> Result<()> {
    todo!()
}

#[e2e::test]
async fn owner_is_beneficiary_and_correct_amount(alice: Account) -> Result<()> {
    // vesting duration should be 30 seconds for easy testing

    let start = timestamp();
    let duration = 30u64;

    let contract_addr = alice
        .as_deployer()
        .with_constructor(vesting_constructor(alice.address(), start, duration))
        .deploy()
        .await?
        .address()?;
    let contract = VestingWallet::new(contract_addr, &alice.wallet);

    let alice_balance = alice.wallet.get_balance(alice.address()).await?;
    let expected = parse_ether("10")?;
    assert_eq!(alice_balance, expected);

    tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
    let _ = watch!(contract.releaseEth())?;

    let alice_balance = alice.wallet.get_balance(alice.address()).await?;
    let expected = parse_ether("20")?;
    assert_eq!(alice_balance, expected);

    Ok(())
}

#[e2e::test]
async fn releasing_correct_erc20_amount_to_beneficiary() -> Result<()> {
    todo!()
}

#[e2e::test]
async fn releasing_both_tokens() -> Result<()> {
    todo!()
}

// changing wallet ownership during vesting schedule will continue to the new
// beneficiary account
#[e2e::test]
async fn changing_ownership_during_vesting_continues(
    alice: Account,
    bob: Account,
) -> Result<()> {
    let start = timestamp();
    let duration = 23_716_800u64;

    let contract_addr = alice
        .as_deployer()
        .with_constructor(vesting_constructor(alice.address(), start, duration))
        .deploy()
        .await?
        .address()?;
    let contract = VestingWallet::new(contract_addr, &alice.wallet);

    let alice_balance = alice.wallet.get_balance(alice.address()).await?;
    let expected = parse_ether("10")?;
    assert_eq!(alice_balance, expected);

    // change ownership
    let _ = watch!(contract.transferOwnership(bob.address()))?;

    // time elapse to unlock tokens
    tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
    let _ = watch!(contract.releaseEth())?;

    // alice amount remains the same as is no longer owner
    let alice_balance = alice.wallet.get_balance(alice.address()).await?;
    let expected = parse_ether("10")?;
    assert_eq!(alice_balance, expected);

    // bob updates balance as the new owner after vesting
    let bob_balance = bob.wallet.get_balance(bob.address()).await?;
    let expected = parse_ether("20")?;
    assert_eq!(bob_balance, expected);

    Ok(())
}
