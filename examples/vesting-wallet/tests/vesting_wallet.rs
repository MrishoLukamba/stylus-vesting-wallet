#![cfg(feature = "e2e")]

mod abi;
use abi::VestingWallet;
use alloy::{
    network::ReceiptResponse,
    primitives::{uint, Address},
    providers::Provider,
    rpc::types::{BlockTransactionsKind, TransactionRequest},
    sol,
};
use alloy_primitives::utils::parse_ether;
use alloy_sol_types::SolConstructor;
use e2e::{
    fund_account, receipt, watch, Account, EventExt, ReceiptExt, Revert,
};
use eyre::Result;
use futures::StreamExt;
use koba::config::Deploy;

use crate::VestingWalletExample::constructorCall;

sol!("src/constructor.sol");

sol!(
    #[sol(rpc)]
    contract ERC20 {
        constructor(string memory name_, string memory symbol_, uint256 cap_);
        function name() external view returns (string name);
        function symbol() external view returns (string symbol);
        function decimals() external view returns (uint8 decimals);
        function balanceOf(address account) external view returns (uint256 balance);
        function transfer(address recipient, uint256 amount) external returns (bool);
        function transferFrom(address sender, address recipient, uint256 amount) external returns (bool);
        function mint(address account, uint256 amount) external;

        #[derive(Debug, PartialEq)]
        event Transfer(address indexed from, address indexed to, uint256 value);
    }
);

async fn erc20_deploy(name: &str, symbol: &str, acc: Account) -> Address {
    let args = ERC20::constructorCall {
        name_: name.to_owned(),
        symbol_: symbol.to_owned(),
        cap_: Default::default(),
    };
    let args = alloy::hex::encode(args.abi_encode());

    let manifest_dir =
        std::env::current_dir().expect("should get current dir from env");

    // Go back two directories
    let target_dir = manifest_dir
        .parent() // Go up one directory
        .expect("Failed to get parent directory") // Handle potential error
        .parent() // Go up another directory
        .expect("Failed to get parent directory");

    let wasm_path = target_dir
        .join("target")
        .join("wasm32-unknown-unknown")
        .join("release")
        .join("erc20_example.wasm");
    let sol_path = target_dir
        .join("examples")
        .join("erc20")
        .join("src")
        .join("constructor.sol");

    let config = Deploy {
        generate_config: koba::config::Generate {
            wasm: wasm_path.clone(),
            sol: Some(sol_path),
            args: Some(args),
            legacy: false,
        },
        auth: koba::config::PrivateKey {
            private_key_path: None,
            private_key: Some(acc.pk()),
            keystore_path: None,
            keystore_password_path: None,
        },
        endpoint: acc.url().to_owned(),
        deploy_only: false,
        quiet: false,
    };

    koba::deploy(&config)
        .await
        .expect("should deploy contract")
        .contract_address()
        .expect("should return contract address")
}

fn vesting_constructor(
    alice: Address,
    start: u64,
    duration: u64,
) -> constructorCall {
    constructorCall {
        beneficiary: alice,
        startTimestamp: start,
        durationSeconds: duration,
    }
}

// getting block starting timestamp for vesting tests
async fn current_timestamp(alice: Account) -> Result<u64> {
    let block_hash = alice
        .wallet
        .watch_blocks()
        .await?
        .into_stream()
        .flat_map(futures::stream::iter)
        .take(1)
        .next()
        .await
        .expect("should get 1st block hash");
    let start = alice
        .wallet
        .get_block_by_hash(block_hash, BlockTransactionsKind::Hashes)
        .await?
        .expect("should return block")
        .header
        .timestamp;
    Ok(start)
}

// ============================================================================
// Integration Tests: VestingWallet
// ============================================================================

#[e2e::test]
async fn constructor(alice: Account) -> Result<()> {
    let start = current_timestamp(alice.clone()).await?;
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
    let start = current_timestamp(alice.clone()).await?;
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

// if duration is set to 0, then vesting contract will act as time lock contract
#[e2e::test]
async fn vesting_acts_like_lock_if_duration_zero(alice: Account) -> Result<()> {
    let timestamp = current_timestamp(alice.clone()).await?;
    let start = timestamp + 5_929_200_u64;
    let duration = 0u64;

    let contract_addr = alice
        .as_deployer()
        .with_constructor(vesting_constructor(alice.address(), start, duration))
        .deploy()
        .await?
        .address()?;
    let contract = VestingWallet::new(contract_addr, &alice.wallet);

    // fund the vesting contract wallet
    fund_account(contract_addr, 10)?;
    // let tx = TransactionRequest::default().from(alice.address()).
    // to(contract_addr).value(uint!(10U256)); let b = alice.wallet.
    // send_transaction(tx).await?;

    // ========================================================================================= //
    let VestingWallet::vestedAmount_0Return { amount } =
        contract.vestedAmount_0(timestamp + 1_000_000).call().await?;
    assert_eq!(amount, uint!(0_U256));
    // when time elapses as it reaches start time the funds unlock
    let VestingWallet::vestedAmount_0Return { amount } =
        contract.vestedAmount_0(start + 1).call().await?;
    assert_eq!(amount, uint!(10_U256));

    Ok(())
}

// testing daily vesting
#[e2e::test]
async fn vesting_schedule_works(alice: Account) -> Result<()> {
    let start = current_timestamp(alice.clone()).await?;
    let day = 86_400u64;
    let duration = day * 5; // 5 days vesting

    let contract_addr = alice
        .as_deployer()
        .with_constructor(vesting_constructor(alice.address(), start, duration))
        .deploy()
        .await?
        .address()?;
    let contract = VestingWallet::new(contract_addr, &alice.wallet);

    // fund the vesting contract wallet
    fund_account(contract_addr, 10)?;

    let VestingWallet::vestedAmount_0Return { amount } =
        contract.vestedAmount_0(day).call().await?;
    assert_eq!(amount, uint!(2_U256));

    let VestingWallet::vestedAmount_0Return { amount } =
        contract.vestedAmount_0(day * 2).call().await?;
    assert_eq!(amount, uint!(4_U256));

    let VestingWallet::vestedAmount_0Return { amount } =
        contract.vestedAmount_0(day * 3).call().await?;
    assert_eq!(amount, uint!(6_U256));

    let VestingWallet::vestedAmount_0Return { amount } =
        contract.vestedAmount_0(day * 4).call().await?;
    assert_eq!(amount, uint!(8_U256));

    let VestingWallet::vestedAmount_0Return { amount } =
        contract.vestedAmount_0(day * 5).call().await?;
    assert_eq!(amount, uint!(10_U256));

    Ok(())
}

// checking if adding eth to an existing vesting schedule will continue and use
// the updated eth amount
#[e2e::test]
async fn add_eth_to_existing_vesting_schedule_continues(
    alice: Account,
) -> Result<()> {
    let start = current_timestamp(alice.clone()).await?;
    let day = 86_400u64;
    let duration = day * 5; // 5 days vesting

    let contract_addr = alice
        .as_deployer()
        .with_constructor(vesting_constructor(alice.address(), start, duration))
        .deploy()
        .await?
        .address()?;
    let contract = VestingWallet::new(contract_addr, &alice.wallet);

    // fund the vesting contract wallet
    fund_account(contract_addr, 10)?;

    let contract_balance = alice.wallet.get_balance(contract_addr).await?;
    let expected = parse_ether("10")?;
    assert_eq!(contract_balance, expected);

    let VestingWallet::vestedAmount_0Return { amount } =
        contract.vestedAmount_0(day).call().await?;
    assert_eq!(amount, uint!(2_U256));

    let VestingWallet::vestedAmount_0Return { amount } =
        contract.vestedAmount_0(day * 2).call().await?;
    assert_eq!(amount, uint!(4_U256));

    // adding eth to the vesting contract
    fund_account(contract_addr, 12)?;

    let contract_balance = alice.wallet.get_balance(contract_addr).await?;
    let expected = parse_ether("22")?;
    assert_eq!(contract_balance, expected);

    // vesting amount changes

    let VestingWallet::vestedAmount_0Return { amount } =
        contract.vestedAmount_0(day * 3).call().await?;
    assert_eq!(amount, uint!(10_U256));

    let VestingWallet::vestedAmount_0Return { amount } =
        contract.vestedAmount_0(day * 4).call().await?;
    assert_eq!(amount, uint!(16_U256));

    let VestingWallet::vestedAmount_0Return { amount } =
        contract.vestedAmount_0(day * 5).call().await?;
    assert_eq!(amount, uint!(22_U256));

    // total amount of eth released
    let VestingWallet::released_0Return { amount } =
        contract.released_0().call().await?;
    assert_eq!(amount, uint!(22_U256));

    Ok(())
}

#[e2e::test]
async fn vesting_erc20_and_eth_works(alice: Account) -> Result<()> {
    // 30 seconds vesting period
    let start = current_timestamp(alice.clone()).await?;
    let duration = 30u64;

    let tk1_contract_addr = erc20_deploy("Token1", "TK1", alice.clone()).await;
    let tk1_contract = ERC20::new(tk1_contract_addr, alice.wallet.clone());

    let contract_addr = alice
        .as_deployer()
        .with_constructor(vesting_constructor(alice.address(), start, duration))
        .deploy()
        .await?
        .address()?;
    let contract = VestingWallet::new(contract_addr, &alice.wallet);

    // fund the vesting contract wallet
    fund_account(contract_addr, 10)?;

    // fund vesting wallet contract with tk1 tokens
    let _ = watch!(tk1_contract.mint(contract_addr, uint!(10_U256)));
    let ERC20::balanceOfReturn { balance } =
        tk1_contract.balanceOf(contract_addr).call().await?;
    assert_eq!(balance, uint!(10_U256));

    // initial state
    // alice tk1 balance
    let ERC20::balanceOfReturn { balance } =
        tk1_contract.balanceOf(alice.address()).call().await?;
    assert_eq!(balance, uint!(0_U256));

    // amount of erc20 tk1 released
    let VestingWallet::released_1Return { amount } =
        contract.released_1(tk1_contract_addr).call().await?;
    assert_eq!(amount, uint!(0_U256));

    // ================================================================== //

    // time elapse to unlock half tokens
    tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;

    // release eth
    let tx_receipt = receipt!(contract.release_0())?;
    assert!(tx_receipt.emits(VestingWallet::EtherReleased {
        beneficiary: alice.address(),
        value: uint!(5_U256),
    }));
    // release erc20 tk1
    let tx_receipt = receipt!(contract.release_1(tk1_contract_addr))?;
    assert!(tx_receipt.emits(VestingWallet::ERC20Released {
        beneficiary: alice.address(),
        token: tk1_contract_addr,
        value: uint!(5_U256),
    }));

    // alice tk1 balance & eth balance
    let ERC20::balanceOfReturn { balance } =
        tk1_contract.balanceOf(alice.address()).call().await?;
    assert_eq!(balance, uint!(5_U256));

    let alice_balance = alice.wallet.get_balance(alice.address()).await?;
    assert_eq!(alice_balance, uint!(5_U256));

    // amount of eth released
    let VestingWallet::released_0Return { amount } =
        contract.released_0().call().await?;
    assert_eq!(amount, uint!(5_U256));
    // amount of erc20 tk1 released
    let VestingWallet::released_1Return { amount } =
        contract.released_1(tk1_contract_addr).call().await?;
    assert_eq!(amount, uint!(5_U256));

    // ================================================================== //

    // time elapse to unlock all tokens
    tokio::time::sleep(tokio::time::Duration::from_secs(20)).await;

    // release eth
    let tx_receipt = receipt!(contract.release_0())?;
    assert!(tx_receipt.emits(VestingWallet::EtherReleased {
        beneficiary: alice.address(),
        value: uint!(5_U256),
    }));
    // release erc20 tk1
    let tx_receipt = receipt!(contract.release_1(tk1_contract_addr))?;
    assert!(tx_receipt.emits(VestingWallet::ERC20Released {
        beneficiary: alice.address(),
        token: tk1_contract_addr,
        value: uint!(5_U256),
    }));

    // alice tk1 balance & eth balance
    let ERC20::balanceOfReturn { balance } =
        tk1_contract.balanceOf(alice.address()).call().await?;
    assert_eq!(balance, uint!(10_U256));

    let alice_balance = alice.wallet.get_balance(alice.address()).await?;
    assert_eq!(alice_balance, uint!(10_U256));

    // amount of eth released
    let VestingWallet::released_0Return { amount } =
        contract.released_0().call().await?;
    assert_eq!(amount, uint!(10_U256));
    // amount of erc20 tk1 released
    let VestingWallet::released_1Return { amount } =
        contract.released_1(tk1_contract_addr).call().await?;
    assert_eq!(amount, uint!(10_U256));

    Ok(())
}

#[e2e::test]
async fn vesting_multiple_erc20_works(alice: Account) -> Result<()> {
    // 30 seconds vesting period
    let start = current_timestamp(alice.clone()).await?;
    let duration = 30u64;

    let tk1_contract_addr = erc20_deploy("Token1", "TK1", alice.clone()).await;
    let tk2_contract_addr = erc20_deploy("Token2", "TK2", alice.clone()).await;

    let tk1_contract = ERC20::new(tk1_contract_addr, alice.wallet.clone());
    let tk2_contract = ERC20::new(tk2_contract_addr, alice.wallet.clone());

    let contract_addr = alice
        .as_deployer()
        .with_constructor(vesting_constructor(alice.address(), start, duration))
        .deploy()
        .await?
        .address()?;
    let contract = VestingWallet::new(contract_addr, &alice.wallet);

    // fund the vesting contract wallet
    fund_account(contract_addr, 10)?;

    // fund the vesting wallet contract with 2 tokens (tk1 & tk2)
    let _ = watch!(tk1_contract.mint(contract_addr, uint!(10_U256)));
    let _ = watch!(tk2_contract.mint(contract_addr, uint!(10_U256)));

    let ERC20::balanceOfReturn { balance } =
        tk1_contract.balanceOf(contract_addr).call().await?;
    assert_eq!(balance, uint!(10_U256));

    let ERC20::balanceOfReturn { balance } =
        tk2_contract.balanceOf(contract_addr).call().await?;
    assert_eq!(balance, uint!(10_U256));

    // initial state
    // alice tk1 balance
    let ERC20::balanceOfReturn { balance } =
        tk1_contract.balanceOf(alice.address()).call().await?;
    assert_eq!(balance, uint!(0_U256));

    // alice tk2 balance
    let ERC20::balanceOfReturn { balance } =
        tk2_contract.balanceOf(alice.address()).call().await?;
    assert_eq!(balance, uint!(0_U256));

    // amount of erc20 tk1 released
    let VestingWallet::released_1Return { amount } =
        contract.released_1(tk1_contract_addr).call().await?;
    assert_eq!(amount, uint!(0_U256));

    // amount of erc20 tk2 released
    let VestingWallet::released_1Return { amount } =
        contract.released_1(tk2_contract_addr).call().await?;
    assert_eq!(amount, uint!(0_U256));

    // =============================================================== //

    // time elapse to unlock half tokens
    tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;

    // release erc20 tk1
    let tx_receipt = receipt!(contract.release_1(tk1_contract_addr))?;
    assert!(tx_receipt.emits(VestingWallet::ERC20Released {
        beneficiary: alice.address(),
        token: tk1_contract_addr,
        value: uint!(5_U256),
    }));
    // release erc20 tk2
    let tx_receipt = receipt!(contract.release_1(tk2_contract_addr))?;
    assert!(tx_receipt.emits(VestingWallet::ERC20Released {
        beneficiary: alice.address(),
        token: tk2_contract_addr,
        value: uint!(5_U256),
    }));

    // alice tk1 & tk2 balance
    let ERC20::balanceOfReturn { balance } =
        tk1_contract.balanceOf(alice.address()).call().await?;
    assert_eq!(balance, uint!(5_U256));

    let ERC20::balanceOfReturn { balance } =
        tk2_contract.balanceOf(alice.address()).call().await?;
    assert_eq!(balance, uint!(5_U256));

    // amount of erc20 tk1 released
    let VestingWallet::released_1Return { amount } =
        contract.released_1(tk1_contract_addr).call().await?;
    assert_eq!(amount, uint!(5_U256));
    // amount of erc20 tk2 released
    let VestingWallet::released_1Return { amount } =
        contract.released_1(tk2_contract_addr).call().await?;
    assert_eq!(amount, uint!(5_U256));

    // ================================================================= //

    // time elapse to unlock all tokens
    tokio::time::sleep(tokio::time::Duration::from_secs(20)).await;

    // release erc20 tk1
    let tx_receipt = receipt!(contract.release_1(tk1_contract_addr))?;
    assert!(tx_receipt.emits(VestingWallet::ERC20Released {
        beneficiary: alice.address(),
        token: tk1_contract_addr,
        value: uint!(5_U256),
    }));
    // release erc20 tk2
    let tx_receipt = receipt!(contract.release_1(tk2_contract_addr))?;
    assert!(tx_receipt.emits(VestingWallet::ERC20Released {
        beneficiary: alice.address(),
        token: tk2_contract_addr,
        value: uint!(5_U256),
    }));

    // alice tk1 & tk2 balances
    let ERC20::balanceOfReturn { balance } =
        tk1_contract.balanceOf(alice.address()).call().await?;
    assert_eq!(balance, uint!(10_U256));

    let ERC20::balanceOfReturn { balance } =
        tk2_contract.balanceOf(alice.address()).call().await?;
    assert_eq!(balance, uint!(10_U256));

    // amount of erc20 tk1 released
    let VestingWallet::released_1Return { amount } =
        contract.released_1(tk1_contract_addr).call().await?;
    assert_eq!(amount, uint!(10_U256));
    // amount of erc20 tk2 released
    let VestingWallet::released_1Return { amount } =
        contract.released_1(tk2_contract_addr).call().await?;
    assert_eq!(amount, uint!(10_U256));
    Ok(())
}

// checking if adding Erc20 token to an existing vesting schedule will continue
// and use the updated Erc20 token amount
#[e2e::test]
async fn add_erc20_to_existing_vesting_schedule_continues(
    alice: Account,
) -> Result<()> {
    // 30 seconds vesting period
    let start = current_timestamp(alice.clone()).await?;
    let duration = 30u64;

    let tk1_contract_addr = erc20_deploy("Token1", "TK1", alice.clone()).await;
    let tk1_contract = ERC20::new(tk1_contract_addr, alice.wallet.clone());

    let contract_addr = alice
        .as_deployer()
        .with_constructor(vesting_constructor(alice.address(), start, duration))
        .deploy()
        .await?
        .address()?;
    let contract = VestingWallet::new(contract_addr, &alice.wallet);

    // fund the vesting contract wallet
    fund_account(contract_addr, 10)?;

    // fund vesting wallet contract with tk1 tokens
    let _ = watch!(tk1_contract.mint(contract_addr, uint!(10_U256)));
    let ERC20::balanceOfReturn { balance } =
        tk1_contract.balanceOf(contract_addr).call().await?;
    assert_eq!(balance, uint!(10_U256));

    // initial state
    // alice tk1 balance
    let ERC20::balanceOfReturn { balance } =
        tk1_contract.balanceOf(alice.address()).call().await?;
    assert_eq!(balance, uint!(0_U256));

    // amount of erc20 tk1 released
    let VestingWallet::released_1Return { amount } =
        contract.released_1(tk1_contract_addr).call().await?;
    assert_eq!(amount, uint!(0_U256));

    // ========================================================================
    // //

    // time elapse to unlock half tokens
    tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;

    let tx_receipt = receipt!(contract.release_1(tk1_contract_addr))?;
    assert!(tx_receipt.emits(VestingWallet::ERC20Released {
        beneficiary: alice.address(),
        token: tk1_contract_addr,
        value: uint!(5_U256),
    }));

    // alice tk1 balance
    let ERC20::balanceOfReturn { balance } =
        tk1_contract.balanceOf(alice.address()).call().await?;
    assert_eq!(balance, uint!(5_U256));

    // amount of erc20 tk1 released
    let VestingWallet::released_1Return { amount } =
        contract.released_1(tk1_contract_addr).call().await?;
    assert_eq!(amount, uint!(5_U256));

    // add Erc20 (tk1) funds to the vesting wallet contract
    let _ = watch!(tk1_contract.mint(contract_addr, uint!(10_U256)));

    // ========================================================================
    // //

    // time elapse to unlock all + added tokens
    tokio::time::sleep(tokio::time::Duration::from_secs(20)).await;

    let tx_receipt = receipt!(contract.release_1(tk1_contract_addr))?;
    assert!(tx_receipt.emits(VestingWallet::ERC20Released {
        beneficiary: alice.address(),
        token: tk1_contract_addr,
        value: uint!(15_U256),
    }));

    // alice tk1 balance
    let ERC20::balanceOfReturn { balance } =
        tk1_contract.balanceOf(alice.address()).call().await?;
    assert_eq!(balance, uint!(20_U256));

    // amount of erc20 tk1 released
    let VestingWallet::released_1Return { amount } =
        contract.released_1(tk1_contract_addr).call().await?;
    assert_eq!(amount, uint!(20_U256));

    Ok(())
}

#[e2e::test]
async fn owner_is_beneficiary_and_correct_amount(alice: Account) -> Result<()> {
    // vesting duration should be 30 seconds for easy testing
    let start = current_timestamp(alice.clone()).await?;
    let duration = 30u64;

    let contract_addr = alice
        .as_deployer()
        .with_constructor(vesting_constructor(alice.address(), start, duration))
        .deploy()
        .await?
        .address()?;
    let contract = VestingWallet::new(contract_addr, &alice.wallet);

    // fund the vesting contract wallet
    fund_account(contract_addr, 10)?;

    let alice_balance = alice.wallet.get_balance(alice.address()).await?;
    let expected = parse_ether("10")?;
    assert_eq!(alice_balance, expected);

    // time elapse to unlock tokens
    tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
    // release eth
    let tx_receipt = receipt!(contract.release_0())?;
    assert!(tx_receipt.emits(VestingWallet::EtherReleased {
        beneficiary: alice.address(),
        value: uint!(10_U256),
    }));

    let alice_balance = alice.wallet.get_balance(alice.address()).await?;
    let expected = parse_ether("20")?;
    assert_eq!(alice_balance, expected);

    // total amount of eth released
    let VestingWallet::released_0Return { amount } =
        contract.released_0().call().await?;
    assert_eq!(amount, uint!(10_U256));

    Ok(())
}

// changing wallet ownership during vesting schedule will continue to the new
// beneficiary account
#[e2e::test]
async fn changing_ownership_during_vesting_continues(
    alice: Account,
    bob: Account,
) -> Result<()> {
    let start = current_timestamp(alice.clone()).await?;
    let duration = 23_716_800u64;

    let contract_addr = alice
        .as_deployer()
        .with_constructor(vesting_constructor(alice.address(), start, duration))
        .deploy()
        .await?
        .address()?;
    let contract = VestingWallet::new(contract_addr, &alice.wallet);

    // fund the vesting contract wallet
    fund_account(contract_addr, 10)?;

    let alice_balance = alice.wallet.get_balance(alice.address()).await?;
    let expected = parse_ether("10")?;
    assert_eq!(alice_balance, expected);

    // change ownership
    let _ = watch!(contract.transferOwnership(bob.address()))?;

    // time elapse to unlock tokens
    tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
    // release eth
    let tx_receipt = receipt!(contract.release_0())?;
    assert!(tx_receipt.emits(VestingWallet::EtherReleased {
        beneficiary: bob.address(),
        value: uint!(10_U256),
    }));

    // alice amount remains the same as is no longer owner
    let alice_balance = alice.wallet.get_balance(alice.address()).await?;
    let expected = parse_ether("10")?;
    assert_eq!(alice_balance, expected);

    // bob updates balance as the new owner after vesting
    let bob_balance = bob.wallet.get_balance(bob.address()).await?;
    let expected = parse_ether("20")?;
    assert_eq!(bob_balance, expected);

    // total amount of eth released
    let VestingWallet::released_0Return { amount } =
        contract.released_0().call().await?;
    assert_eq!(amount, uint!(10_U256));

    Ok(())
}
