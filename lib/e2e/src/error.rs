use alloy::sol_types::SolError;

pub trait ErrorExt<E> {
    /// Checks that `Self` corresponds to the typed abi-encoded error
    /// `expected`.
    fn is_err(&self, expected: E) -> bool;
}

impl<E: SolError> ErrorExt<E> for alloy::contract::Error {
    fn is_err(&self, expected: E) -> bool {
        let Self::TransportError(e) = self else {
            return false;
        };

        let raw_value = e
            .as_error_resp()
            .and_then(|payload| payload.data.clone())
            .expect("should extract the error");
        let actual = &raw_value.get().trim_matches('"')[2..];
        let expected = alloy::hex::encode(expected.abi_encode());
        expected == actual
    }
}

impl<E: SolError> ErrorExt<E> for eyre::ErrReport {
    fn is_err(&self, expected: E) -> bool {
        // TODO: improve error check
        // Requires strange casting
        //  ErrorResp(
        //      ErrorPayload {
        //          code: 3,
        //          message: \"execution reverted\",
        //          data: Some(
        //              RawValue(
        //                  \"0x...\",
        //              ),
        //          ),
        //      },
        //  )
        let err_string = format!("{:#?}", self);
        let expected = alloy::hex::encode(expected.abi_encode());
        err_string.contains(&expected)
    }
}
