#![allow(dead_code)]
#![allow(clippy::too_many_arguments)]
use alloy::sol;

sol!(
    #[sol(rpc)]
    contract VestingWallet {
        function receiveEth() external payable;

        function releaseEth() external;

        function releaseErc20(address token) external;

        function vestedEthAmount(uint64 timestamp) external view returns (uint256 amount);

        function vestedErc20Amount(address token, uint64 timestamp) external returns (uint256 amount);

        function start() external view returns (uint256 start);

        function duration() external view returns (uint256 duration);

        function end() external view returns (uint256 end);

        function releasedEth() external view returns (uint256 amount);

        function releasedErc20(address token) external view returns (uint256 amount);

        function releasableEth() external view returns (uint256 amount);

        function releasableErc20(address token) external returns (uint256 amount);

        #[derive(Debug, PartialEq)]
        event EtherReleased(address indexed beneficiary, uint256 value);

        #[derive(Debug, PartialEq)]
        event ERC20Released(address indexed beneficiary, address indexed token, uint256 value);

        error FailedToDecode();

        error RemoteContractCallFailed();

        error FailedToEncodeValue();

        function owner() external view returns (address owner);

        function onlyOwner() external view;

        function transferOwnership(address new_owner) external;

        function renounceOwnership() external;

        error OwnableUnauthorizedAccount(address);

        error OwnableInvalidOwner(address owner);

        #[derive(Debug, PartialEq)]
        event OwnershipTransferred(address indexed previousOwner, address indexed newOwner);
    }
);
