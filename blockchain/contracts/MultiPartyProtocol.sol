pragma solidity ^0.4.11;

contract MultiPartyProtocol {

    struct PlayerCommitment {
        bool initialized;
        bytes32 commitment;
        bytes nizks;
        bytes publicKey;
    }

    struct StageCommit {
        mapping (address => PlayerCommitment) playerCommitments;
        bytes32 hashOfAllCommitments;    // == hash of all commitments
        uint commitmentLength;
    }

    struct StageTransform {
        mapping (address => bytes) playerCommitments;
    }

    struct Keypair {
        bytes provingKey;
        bytes verificationKey;
    }
    
    struct Protocol {
        string r1cs;
        bytes[] initialStages;         //before round robin, the initial stage is stored here, starting with stage 1
        StageCommit stageCommit;
        StageTransform[] stageTransformations;
        Keypair keypair;
    }
    
    event PlayerJoined(address player);    //called when a player joined the protocol
    event PlayerCommitted(address player, bytes32 commitment); //called when a player committed hash of public key
    event NextStage(uint stage);  //called when a new stage begins
    event StagePrepared(uint stage);  //called when the coordinator initialized a new stage (stage1, stage2, stage3)
    event StageResultPublished(address player, bytes result);
    
    modifier isCoordinator(){
        require(msg.sender == players[0]);
        _;
    }

    modifier isEmpty(string s){
        require(isStringEmpty(s));
        _;
    }
    
    modifier isEmptyBytes(bytes h){
        require(h.length == 0);
        _;
    }

    modifier isEmptyBytes32(bytes32 h){
        require(h == "");
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
    
    modifier isNotEmptyBytes(bytes h){
        require(h.length > 0);
        _;
    }

    modifier isNotEmptyBytes32(bytes32 h){
        require(h.length > 0);
        _;
    }
    
    modifier isInState(State s){
        require(currentState == s);
        _;
    }

    modifier isInStageTransformationState(){
        require(
            currentState == State.Stage1 || 
            currentState == State.Stage2 || 
            currentState == State.Stage3);
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
            bytes storage initialStage = protocol.initialStages[stageIndex];
            require(!isBytesEmpty(initialStage));
        } else {
            require(
                !isBytesEmpty(
                    protocol.stageTransformations[stageIndex]
                    .playerCommitments[players[pIndex-1]]
                )
            );
        }
        _;
    }
    
    enum State {Join, Commit, Nizks, Stage1, Stage2, Stage3, Finished}
    State public currentState = State.Join;
    address[] public players;
    Protocol protocol;
    
    function MultiPartyProtocol(string r1cs) public {
        protocol.r1cs = r1cs;
        protocol.initialStages = new bytes[](3);
        protocol.stageCommit = StageCommit("", 0);
        protocol.stageTransformations.push(StageTransform());
        protocol.stageTransformations.push(StageTransform());
        protocol.stageTransformations.push(StageTransform());
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
            if(!protocol.stageCommit.playerCommitments[players[i]].initialized ||
            protocol.stageCommit.playerCommitments[players[i]].commitment.length == 0){
                return false;
            }
        }
        return true;
    }

    function allPlayerDataReady() constant internal returns (bool) {
        for(uint i = 0; i < players.length; i++){
            bytes memory nizks = protocol.stageCommit.playerCommitments[players[i]].nizks;
            bytes memory pubKey = protocol.stageCommit.playerCommitments[players[i]].publicKey;
            if(!protocol.stageCommit.playerCommitments[players[i]].initialized 
            || isBytesEmpty(nizks) 
            || isBytesEmpty(pubKey)){
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

    function isLastPlayer() constant internal returns (bool) {
        return players[players.length - 1] == msg.sender;
    }

    function hashAllCommitments() view internal returns (bytes32) {
        uint keysize = 32;
        bytes memory allCommitments = new bytes(keysize*players.length);
        for(uint i = 0; i < players.length; i++){
            bytes32 commitment = protocol.stageCommit.playerCommitments[players[i]].commitment;
            for(uint j = 0; j < keysize; j++){
                allCommitments[(i*keysize)+j] = commitment[j];
            }
        }
        return keccak256(allCommitments);
        /*
        mapping(address => PlayerCommitment) commitments = protocol.stageCommit.playerCommitments;
        assembly {
            let offset := 0
            let totalOffset := 0
            let p := sload(keccak256(players_slot, players_offset))
            let concatCommitments := mload(0x40)                                                    //empty storage pointer
            for{let i := 0} lt(i, players_slot) {i := add(i, 0x1)} {                                  //for each player
                let commitments := sload(commitments_slot)                                          //load commitment
                for{} lt(offset, 0x7F7) {offset := add(offset, 0x20)}{                              //for all words in pubkey
                    mstore(add(concatCommitments, add(totalOffset, offset)), add(commitments, offset))   //append word to new array
                }
                totalOffset := add(totalOffset, 0x7F7)                                              //add total offset for next player
                sstore(concatCommitments, result)
            } 
        }*/
        
    }

    function isStringEmpty(string s) pure internal returns (bool) {
        return isBytesEmpty(bytes(s));
    }

    function isBytesEmpty(bytes b) pure internal returns (bool) {
        return b.length == 0;
    }
}