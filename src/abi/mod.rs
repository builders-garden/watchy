use alloy::sol;

// EIP-8004 Identity Registry contract interface
// Based on IdentityRegistryUpgradeable
sol! {
    #[sol(rpc)]
    interface IIdentityRegistry {
        // ERC-721 standard
        function ownerOf(uint256 tokenId) external view returns (address);
        function balanceOf(address owner) external view returns (uint256);
        function tokenURI(uint256 tokenId) external view returns (string memory);

        // EIP-8004 specific
        function getAgentWallet(uint256 agentId) external view returns (address);
        function getMetadata(uint256 agentId, string calldata metadataKey) external view returns (bytes memory);
        function isAuthorizedOrOwner(address spender, uint256 agentId) external view returns (bool);

        // Registration (for reference, not used by Watchy)
        function register() external returns (uint256 agentId);
        function register(string calldata agentURI) external returns (uint256 agentId);

        // Events
        event Registered(uint256 indexed agentId, string agentURI, address indexed owner);
        event URIUpdated(uint256 indexed agentId, string newURI, address indexed updatedBy);
        event MetadataSet(uint256 indexed agentId, string indexed indexedMetadataKey, string metadataKey, bytes metadataValue);

        // Errors
        error ERC721NonexistentToken(uint256 tokenId);
    }
}

// EIP-8004 Reputation Registry contract interface
sol! {
    #[sol(rpc)]
    interface IReputationRegistry {
        // Give feedback for an agent
        function giveFeedback(
            uint256 agentId,
            int128 value,
            uint8 valueDecimals,
            string calldata tag1,
            string calldata tag2,
            string calldata endpoint,
            string calldata feedbackURI,
            bytes32 feedbackHash
        ) external;

        // Revoke feedback
        function revokeFeedback(uint256 agentId, uint64 feedbackIndex) external;

        // Get feedback count for a client-agent pair
        function getFeedbackCount(address clientAddress, uint256 agentId) external view returns (uint64);

        // Events
        event NewFeedback(
            uint256 indexed agentId,
            address indexed clientAddress,
            uint64 feedbackIndex,
            int128 value,
            uint8 valueDecimals,
            string indexed indexedTag1,
            string tag1,
            string tag2,
            string endpoint,
            string feedbackURI,
            bytes32 feedbackHash
        );

        event FeedbackRevoked(
            uint256 indexed agentId,
            address indexed clientAddress,
            uint64 feedbackIndex
        );
    }
}

