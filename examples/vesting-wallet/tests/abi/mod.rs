#![allow(dead_code)]
#![allow(clippy::too_many_arguments)]
use alloy::sol;

sol!(
    #[sol(rpc)]
    contract VestingWallet {
        function receiveEth() external payable;
    
        function releaseEth() external;
    
        function releaseErc20(address token) external;
    
        function vestedEthAmount(uint64 timestamp) external view returns (uint256);
    
        function vestedErc20Amount(address token, uint64 timestamp) external returns (uint256);
    
        function start() external view returns (uint256);
    
        function duration() external view returns (uint256);
    
        function end() external view returns (uint256);
    
        function releasedEth() external view returns (uint256);
    
        function releasedErc20(address token) external view returns (uint256);

        #[derive(Debug, PartialEq)]
        event EtherReleased(address indexed beneficiary, uint256 value);

        #[derive(Debug, PartialEq)]
        event ERC20Released(address indexed beneficiary, address indexed token, uint256 value);

        error FailedToDecode();
    
        error RemoteContractCallFailed();
    
        error FailedToEncodeValue();
    
        function owner() external view returns (address);
    
        function onlyOwner() external view;
    
        function transferOwnership(address new_owner) external;
    
        function renounceOwnership() external;
    
        error OwnableUnauthorizedAccount(address);
    
        error OwnableInvalidOwner(address);
    
    }
);

