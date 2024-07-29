//! Elliptic Curve Digital Signature Algorithm (ECDSA) operations.
//!
//! These functions can be used to verify that a message was signed
//! by the holder of the private keys of a given address.
use alloc::vec::Vec;

use alloy_primitives::{
    address, fixed_bytes, uint, Address, Bytes, B256, U256,
};
use alloy_sol_types::{sol, SolType};
use stylus_proc::SolidityError;
use stylus_sdk::{
    call::{self, Call},
    storage::TopLevelStorage,
};

const SIGNATURE_LENGTH: usize = 65;

const ECRECOVER_ADDR: Address =
    address!("0000000000000000000000000000000000000001");

const VS_MASK: B256 = fixed_bytes!(
    "7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
);

const EIP2_VALUE: U256 = uint!(
    0x7FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF5D576E7357A4501DDFE92F46681B20A0_U256
);

sol! {
    /// The signature derives the `Address::ZERO`.
    #[derive(Debug)]
    #[allow(missing_docs)]
    error ECDSAInvalidSignature();

    /// The signature has an invalid length.
    ///
    /// * `length` - Length of the signature.
    #[derive(Debug)]
    #[allow(missing_docs)]
    error ECDSAInvalidSignatureLength(uint256 length);

    /// The signature has an `S` value that is in the upper half order.
    ///
    /// * `s` - Invalid `S` value.
    #[derive(Debug)]
    #[allow(missing_docs)]
    error ECDSAInvalidSignatureS(bytes32 s);
}

/// An error that occurred in the implementation of an [`ECDSA`] library.
#[derive(SolidityError, Debug)]
pub enum Error {
    /// The signature derives the `Address::ZERO`.
    InvalidSignature(ECDSAInvalidSignature),
    /// The signature has an invalid length.
    InvalidSignatureLength(ECDSAInvalidSignatureLength),
    /// The signature has an `S` value that is in the upper half order.
    InvalidSignatureS(ECDSAInvalidSignatureS),
}

sol! {
    /// Struct with callable data to the `ecrecover` precompile.
    #[allow(missing_docs)]
    struct ECRecoverData {
        /// EIP-191 Hash of the message.
        bytes32 hash;
        /// `v` value from the signature.
        uint8 v;
        /// `r` value from the signature.
        bytes32 r;
        /// `s` value from the signature.
        bytes32 s;
    }
}

/// Returns the address that signed a hashed message (`hash`) with
/// `signature`. This address can then be used for verification purposes.
///
/// IMPORTANT: `hash` _must_ be the result of a hash operation for the
/// verification to be secure: it is possible to craft signatures that
/// recover to arbitrary addresses for non-hashed data.
///
/// # Arguments
///
/// * `storage` - Write access to storage.
/// * `signature` - Signature of the message.
///
/// # Errors
///
/// * If the extracted `s` value is grater than `EIP2_VALUE`, then the error
/// [`Error::ECDSAInvalidSignatureS`] is returned.
/// * If the recovered address is `Address::ZERO`, then the error
/// [`Error::InvalidSignature`] is returned.
///
/// # Panics
///
/// * If `ecrecover` precompile fail to execute.
#[allow(clippy::needless_pass_by_value)]
pub fn recover_from_signature(
    storage: &mut impl TopLevelStorage,
    hash: B256,
    signature: Bytes,
) -> Result<Address, Error> {
    let signature_len = signature.len();
    if signature_len != SIGNATURE_LENGTH {
        return Err(ECDSAInvalidSignatureLength {
            length: U256::from(signature_len),
        }
        .into());
    }

    // extract `r`, `s`, and `v` from the signature
    let r: B256 = signature[0..32]
        .try_into()
        .expect("signature should contain `r` value");

    let s: B256 = signature[32..64]
        .try_into()
        .expect("signature should contain `s` value");

    let v: u8 = signature[64];

    recover(storage, hash, v, r, s)
}

/// Returns the address that signed a hashed message (`hash`).
///
/// [`ECDSA::recover`] that receives the `r` and `vs`
/// short-signature fields separately.
///
/// # Arguments
///
/// * `storage` - Write access to storage.
/// * `hash` - Hash of the message.
/// * `vs` - `vs` value from the signature.
/// * `r` - `r` value from the signature.
///
/// # Errors
///
/// * If the extracted `s` value is grater than `EIP2_VALUE`, then the error
/// [`Error::ECDSAInvalidSignatureS`] is returned.
/// * If the recovered address is `Address::ZERO`, then the error
/// [`Error::InvalidSignature`] is returned.
///
/// # Panics
///
/// * If `ecrecover` precompile fail to execute.
pub fn recover_from_r_vs(
    storage: &mut impl TopLevelStorage,
    hash: B256,
    r: B256,
    vs: B256,
) -> Result<Address, Error> {
    let s: B256 = vs & VS_MASK;
    let v: u8 = (vs[31] >> 7) + 27u8;

    let recovered = recover(storage, hash, v, r, s)?;

    Ok(recovered)
}

/// Returns the address that signed a hashed message (`hash`).
///
/// [`ECDSA::recover`] that receives the `v`,`r` and `s`
/// signature fields separately.
///
/// # Arguments
///
/// * `storage` - Write access to storage.
/// * `hash` - Hash of the message.
/// * `v` - `v` value from the signature.
/// * `r` - `r` value from the signature.
/// * `s` - `s` value from the signature.
///
/// # Errors
///
/// * If the `s` value is grater than `EIP2_VALUE`, then the error
/// [`Error::ECDSAInvalidSignatureS`] is returned.
/// * If the recovered address is `Address::ZERO`, then the error
/// [`Error::InvalidSignature`] is returned.
///
/// # Panics
///
/// * If `ecrecover` precompile fail to execute.
pub fn recover(
    storage: &mut impl TopLevelStorage,
    hash: B256,
    v: u8,
    r: B256,
    s: B256,
) -> Result<Address, Error> {
    // EIP-2 still allows signature malleability for ecrecover().
    //
    // Remove this possibility and make the signature unique.
    //
    // Appendix F in the Ethereum Yellow paper
    // (https://ethereum.github.io/yellowpaper/paper.pdf),
    // defines the valid range for s in (301): 0 < s < secp256k1n ÷ 2 + 1,
    // and for v in (302): v ∈ {27, 28}.
    //
    // Most signatures from current libraries generate a unique signature
    // with an s-value in the lower half order.
    //
    // If your library generates malleable signatures,
    // such as s-values in the upper range, calculate a new s-value
    // with 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141 -
    // s1, and flip v from 27 to 28 or vice versa.
    //
    // If your library also generates signatures with 0/1 for v instead 27/28,
    // add 27 to v to accept these malleable signatures as well.
    if U256::from_be_slice(s.as_slice()) > EIP2_VALUE {
        return Err(ECDSAInvalidSignatureS { s: *s }.into());
    }

    // If the signature is valid (and not malleable), return the signer address.
    let recovered = evm_recover(storage, hash, v, r, s)?;

    Ok(recovered)
}

/// Calls `ecrecover` EVM precompile.
/// The `ecrecover` EVM precompile allows for malleable (non-unique) signatures:
/// this function rejects them by requiring the `s` value to be in the lower
/// half order, and the `v` value to be either 27 or 28.
///
/// # Arguments
///
/// * `storage` - Write access to storage.
/// * `hash` - Hash of the message.
/// * `v` - `v` value from the signature.
/// * `r` - `r` value from the signature.
/// * `s` - `s` value from the signature.
///
/// # Errors
///
/// * If the `s` value is grater than `EIP2_VALUE`, then the error
/// [`Error::ECDSAInvalidSignatureS`] is returned.
/// * If the recovered address is `Address::ZERO`, then the error
/// [`Error::InvalidSignature`] is returned.
///
/// # Panics
///
/// * If `ecrecover` precompile fail to execute.
fn evm_recover(
    storage: &mut impl TopLevelStorage,
    hash: B256,
    v: u8,
    r: B256,
    s: B256,
) -> Result<Address, Error> {
    let calldata = prepare_calldata(hash, v, r, s);

    let recovered =
        call::static_call(Call::new_in(storage), ECRECOVER_ADDR, &calldata)
            .expect("should call `ecrecover` precompile");

    let recovered = B256::from_slice(&recovered);
    let recovered = Address::from_word(recovered);

    if recovered.is_zero() {
        return Err(ECDSAInvalidSignature {}.into());
    }
    Ok(recovered)
}

fn prepare_calldata(hash: B256, v: u8, r: B256, s: B256) -> Vec<u8> {
    let calldata = ECRecoverData { hash: *hash, v, r: *r, s: *s };
    ECRecoverData::encode(&calldata)
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use alloy_primitives::{b256, B256};

    const MSG_HASH: B256 = b256!(
        "a1de988600a42c4b4ab089b619297c17d53cffae5d5120d82d8a92d0bb3b78f2"
    );
    const V: u8 = 28;
    const R: B256 = b256!(
        "65e72b1cf8e189569963750e10ccb88fe89389daeeb8b735277d59cd6885ee82"
    );
    const S: B256 = b256!(
        "3eb5a6982b540f185703492dab77b863a88ce01f27e21ade8b2879c10fc9e653"
    );

    #[test]
    fn prepare_calldata() {
        let expected = alloy_primitives::bytes!("a1de988600a42c4b4ab089b619297c17d53cffae5d5120d82d8a92d0bb3b78f2000000000000000000000000000000000000000000000000000000000000001c65e72b1cf8e189569963750e10ccb88fe89389daeeb8b735277d59cd6885ee823eb5a6982b540f185703492dab77b863a88ce01f27e21ade8b2879c10fc9e653");
        let calldata = super::prepare_calldata(MSG_HASH, V, R, S);
        assert_eq!(expected, calldata);
    }
}
