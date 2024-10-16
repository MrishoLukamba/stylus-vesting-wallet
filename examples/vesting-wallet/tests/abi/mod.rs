#![allow(dead_code)]
#![allow(clippy::too_many_arguments)]
use alloy::sol;

sol!(
    #[sol(rpc)]
    contract VestingWallet {
        function receive() external payable;

        function release() external;

        function release(address token) external;

        function vestedAmount(uint64 timestamp) external view returns (uint256 amount);

        function vestedAmount(address token, uint64 timestamp) external returns (uint256 amount);

        function start() external view returns (uint256 start);

        function duration() external view returns (uint256 duration);

        function end() external view returns (uint256 end);

        function released() external view returns (uint256 amount);

        function released(address token) external view returns (uint256 amount);

        function releasable() external view returns (uint256 amount);

        function releasable(address token) external returns (uint256 amount);

        function owner() external view returns (address owner);

        function onlyOwner() external view;

        function transferOwnership(address new_owner) external;

        function renounceOwnership() external;

        #[derive(Debug, PartialEq)]
        event EtherReleased(address indexed beneficiary, uint256 value);

        #[derive(Debug, PartialEq)]
        event ERC20Released(address indexed beneficiary, address indexed token, uint256 value);

        error FailedToDecode();

        error RemoteContractCallFailed();

        error FailedToEncodeValue();

        error OwnableUnauthorizedAccount(address);

        error OwnableInvalidOwner(address owner);

        #[derive(Debug, PartialEq)]
        event OwnershipTransferred(address indexed previousOwner, address indexed newOwner);
    }
);
