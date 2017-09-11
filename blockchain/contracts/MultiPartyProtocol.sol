pragma solidity ^0.4.0;

import "./strings.sol";

contract MultiPartyProtocol {
    using strings for *;
    struct Commitment {
        string payload;
        bytes32 lastMessageHash;
    }

    struct PlayerCommitment {
        string commitment;
        string nizks;
        string publicKey;
        string iHash;
    }

    struct StageCommit {
        mapping (address => PlayerCommitment) playerCommitments;
        bytes32 lastMessageHash;    // == hash of all commitments
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
        StageCommit stageCommit;
        StageTransform[] stageTransformations;
        Keypair keypair;
    }
    
    event PlayerJoined(address);    //called when a player joined the protocol
    event PlayerCommitted(address, string); //called when a player committed hash of public key
    event NextStage(uint);  //called when a new stage begins
    event StagePrepared(uint);  //called when the coordinator initialized a new stage (stage1, stage2, stage3)
    
    modifier isCoordinator(){
        require(msg.sender == players[0]);
        _;
    }

    modifier isEmpty(string s){
        require(isStringEmpty(s));
        _;
    }

    modifier isNewPlayer (){
        for(uint i = 0; i < players.length; i++){
            require(players[i] != msg.sender);
        }
        _;
    }

    modifier isNotEmpty(string s){
        require(!isStringEmpty(s));
        _;
    }
    
    modifier isInState(State s){
        require(currentState == s);
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

    modifier previousPlayerCommitted() {
        uint pIndex = getPlayerIndex();
        require(
            currentState == State.Stage1
            || currentState == State.Stage2
            || currentState == State.Stage3
        );
        uint stageIndex = uint(currentState) - uint(State.Stage1);
        if(pIndex == 0){
            require(!isStringEmpty(protocol.initialStages[stageIndex]));
        } else {
            require(
                !isStringEmpty(
                    protocol.stageTransformations[stageIndex]
                    .playerCommitments[players[pIndex-1]]
                    .payload
                )
            );
        }
        _;
    }
    
    enum State {Join, Commit, Nizks, Stage1, Stage2, Stage3, Finished}
    State public currentState = State.Join;
    address[] public players;
    Protocol protocol;
    
    function MultiPartyProtocol(string r1cs) {
        protocol.r1cs = r1cs;
        protocol.initialStages = new string[](3);
        protocol.stageCommit = StageCommit("");
        protocol.stageTransformations[0] = StageTransform();
        protocol.stageTransformations[1] = StageTransform();
        protocol.stageTransformations[2] = StageTransform();
        protocol.keypair = Keypair("", "");
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

    
    function allCommitmentsReady() constant internal returns (bool) {
        for(uint i = 0; i < players.length; i++){
            if(isStringEmpty(protocol.stageCommit.playerCommitments[players[i]].commitment)){
                return false;
            }
        }
        return true;
    }

    function allPlayerDataReady() constant internal returns (bool) {
        for(uint i = 0; i < players.length; i++){
            string memory nizks = protocol.stageCommit.playerCommitments[players[i]].nizks;
            string memory pubKey = protocol.stageCommit.playerCommitments[players[i]].publicKey;
            if(isStringEmpty(nizks) || isStringEmpty(pubKey)){
                return false;
            }
        }
        return true;
    }

    function getPlayerIndex() constant internal returns (uint) {
        for(uint i = 0; i < players.length; i++){
            if(players[i] == msg.sender){
                return i;
            }
        }
        require(false);
    }

    function hashAllCommitments() constant internal returns (bytes32) {
        string memory allCommitments = "";
        for(uint i; i < players.length; i++){
            allCommitments = allCommitments.toSlice().concat(
                protocol.stageCommit.playerCommitments[players[i]].commitment.toSlice()
            );
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

    function isStringEmpty(string s) constant internal returns (bool) {
        return bytes(s).length == 0;
    }

    function stringToBytes32(string memory source) constant internal returns (bytes32 result) {
        assembly {
            result := mload(add(source, 32))
        }
    }
}