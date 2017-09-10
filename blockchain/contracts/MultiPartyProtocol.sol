pragma solidity ^0.4.0;

import "./strings.sol";

contract MultiPartyProtocol {
    using strings for *;
    struct Commitment {
        string binaryContent;
        bytes32 lastMessageHash;
    }

    struct PlayerStage1 {
        string nizks;
        string publicKey;
        Commitment commitment;
    }

    struct StageCommit {
        mapping (address => string) playerCommitments;
        bytes32 lastMessageHash;    // == hash of all commitments
    }

    struct StageNizks {
        mapping (address => PlayerStage1) playerCommitments;
    }

    struct StageTransform {
        mapping (address => Commitment) playerCommitments;
    }

    struct Keypair {
        string provingKey;
        string verificationKey;
    }
    
    struct Protocol {
        string r1cs;
        string[] initialStages;         //before round robin, the initial stage is stored here, starting with stage 1
        StageCommit stage0;
        StageNizks stage1;
        StageTransform stage2;
        StageTransform stage3;
        Keypair keypair;
    }
    
    event PlayerJoined(address);    //called when a player joined the protocol
    event PlayerCommitted(address, string); //called when a player committed hash of public key
    event NextStage(uint);  //called when a new stage begins
    event StagePrepared(uint);  //called when the coordinator initialized a new stage (stage1, stage2, stage3)
    
    modifier isEmpty(string s){
        require(isStringEmpty(s));
        _;
    }

    modifier isNotEmpty(string s){
        require(!isStringEmpty(s));
        _;
    }
    
    modifier isNewPlayer (){
        for(uint i = 0; i < players.length; i++){
            require(players[i] != msg.sender);
        }
        _;
    }

    modifier isPlayer (){
        bool found = false;
        for(uint i = 0; i < players.length; i++){
            if(players[i] == msg.sender){
                found = true;
                break;
            }
        }
        require(found);
        _;
    }
    
    modifier isInState(State s){
        require(currentState == s);
        _;
    }
    
    modifier isCoordinator(){
        require(msg.sender == players[0]);
        _;
    }
    
    enum State {Join, Commit, Stage1, Stage2, Stage3, Finished}
    State public currentState = State.Join;
    address[] public players;
    Protocol protocol;
    
    function MultiPartyProtocol(string r1cs) {
        protocol = Protocol(
            r1cs,
            new string[](3), 
            StageCommit(""), 
            StageNizks(), 
            StageTransform(), 
            StageTransform(), 
            Keypair("", "")
        );
    }
    
    function nextStage() internal returns (bool){
        if(currentState != State.Finished){
            currentState = State(uint(currentState) + 1);
            NextStage(uint(currentState));
            return true;
        } else {
            return false;
        }
    }

    function hashAllCommitments() constant internal returns (bytes32) {
        string memory allCommitments = "";
        for(uint i; i < players.length; i++){
            allCommitments = allCommitments.toSlice().concat(protocol.stage0.playerCommitments[players[i]].toSlice());
        }
        return sha3(allCommitments);
    }

    function hashStageOneResults(
        string pubKey, 
        string nizks, 
        string s1Transformed, 
        string iHash
    ) 
        constant 
        internal 
        returns (bytes32) 
    {
        return sha3(
            pubKey.toSlice()
            .concat(nizks.toSlice()).toSlice()
            .concat(s1Transformed.toSlice()).toSlice()
            .concat(iHash.toSlice())
        );
    }
    
    function allCommitmentsReady() constant internal returns (bool) {
        for(uint i = 0; i < players.length; i++){
            if(isStringEmpty(protocol.stage0.playerCommitments[players[i]])){
                return false;
            }
        }
        return true;
    }

    function stringToBytes32(string memory source) constant internal returns (bytes32 result) {
        assembly {
            result := mload(add(source, 32))
        }
    }

    function isStringEmpty(string s) constant internal returns (bool) {
        return bytes(s).length == 0;
    }
}