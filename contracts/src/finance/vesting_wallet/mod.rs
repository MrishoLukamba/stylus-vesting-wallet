//! Implementation of `VESTING_WALLET`
//!
//! This contract has referenced the `OpenZeppelin` vesting_wallet
//! implementation but improved on the following aspect;
//! - Allowing multiple beneficiaries per wallet

// =============================== Implementation draft
// ===========================================

// design consideration

// - the implementation wil be based on trait oriented programming, and allowing
//   easy extensions

// dependency libraries
// - ERC-20, as allowing different ERC-20 token in vesting plans

// - enabling the contract to have multiple beneficiaries each with vesting
//   schedule and token in place
//      - this will have each beneficiary with token,amount and duration vested
//        info in place
//      - beneficiary can have multiple vesting schedules // extra feature
//      - the limitation will be , only 1 token can be vested per be schedule

// - the contract should allow beneficiary to update their EOA, and the vesting
//   should continue as normal

// - vesting will be having different schedules all dictated by curves as
//   backend , i.e, quadratic curves, linear ( as this is the default), etc ..

// - the contract will have a mapping of ERC-20 token addresses to total amount
//   as it allows multiple tokens to be vested

// - all OpenZeppelin vesting_wallet functions implemented are applicable here

// - things to answer, as I dont have clear path for now is;
//      - to handle adjustable supply tokens reflection on vesting schedules
//      - vesting admin -> controller of the vesting contract, as this can be a
//        single account or an account which is controlled by governance

// =================================================================================================

// pseudo code

// important data structure

// struct VestingInfo -> (starting timestamp, duration, ERC-20 token, total
// allocated amount, total amount vested, schedule curve )

// storage
//  - vesting_admin: Address,

//  - mapping( address token => amount) _amount // indicating multiple supported
//    and allocated ERC-20 tokens for vesting in the contract

//  - mapping( address token => amount) _released // indicating total amount
//    released per token throughout the contract

//  - mapping( VestingInfo => address)

// constructor
//  - the initial beneficiary and the vesting schedule should be set, and the
//    admin of the contract

// errors
// - all OpenZeppelin errors specified in vesting-wallet implementation are
//   applicable and any other error will be introduced based on the function
//   implementation

// events
//  - all OpenZeppelin events specified in vesting-wallet implementation are
//    applicable and any other events will be introduced based on the function
//    implementation

// ================================================================================================
// getter functions
//  - all getter functions should specify which beneficiary address to return
//    the info about

// key setter functions
//  - receive_eth()
//  - receive_erc20(token address)
//  - add_vesting_schedule(beneficiary)

// key pure functions ( computation )
// as every vesting schedule curve will be based on one trait which will
// calculate and return
//  - remaining_vesting_schedule,
