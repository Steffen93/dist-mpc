pragma solidity ^0.4.11;

contract MultiPartyProtocol {

    struct PlayerData {
        bool initialized;
        bytes32 commitment;
        bytes nizks;
        bytes publicKey;
    }

    struct StageCommit {
        mapping (address => PlayerData) playerData;
    }

    struct StageTransform {
        mapping (address => bytes) playerData;
    }

    struct Keypair {
        bytes provingKey;
        bytes verificationKey;
    }
    
    struct Protocol {
        bytes r1cs;
        bytes[] initialStages;         //before round robin, the initial stage is stored here, starting with stage 1
        StageCommit stageCommit;
        StageTransform[] stageTransformations;
        Keypair keypair;
        bytes latestTransformation;
    }
    
    event PlayerJoined(address player);    //called when a player joined the protocol
    event PlayerCommitted(address player, bytes32 commitment); //called when a player committed hash of public key
    event NextStage(uint stage);  //called when a new stage begins
    event StagePrepared(uint stage);  //called when the coordinator initialized a new stage (stage1, stage2, stage3)
    event StageResultPublished(address player, bytes result);
    
    modifier isSenderCoordinator(){
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
        require(isInTransformationStage());
        _;
    }
    
    modifier isSenderPlayer (){
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
        require(isInTransformationStage());
        uint stageIndex = uint(currentState) - uint(State.Stage1);
        if(pIndex == 0){
            bytes storage initialStage = protocol.initialStages[stageIndex];
            require(!isBytesEmpty(initialStage));
        } else {
            require(
                !isBytesEmpty(
                    protocol.stageTransformations[stageIndex]
                    .playerData[players[pIndex-1]]
                )
            );
        }
        _;
    }
    
    enum State {Join, Commit, Reveal, Nizks, Stage1, Stage2, Stage3, Finished}
    State public currentState = State.Join;
    address[] public players;
    Protocol protocol;
    
    function MultiPartyProtocol(bytes r1cs) public {
        protocol.r1cs = r1cs;
        protocol.initialStages = new bytes[](3);
        protocol.stageCommit = StageCommit();
        protocol.stageTransformations.push(StageTransform());
        protocol.stageTransformations.push(StageTransform());
        protocol.stageTransformations.push(StageTransform());
        protocol.keypair = Keypair("", "");
        protocol.latestTransformation = "";
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
            if(!protocol.stageCommit.playerData[players[i]].initialized ||
            protocol.stageCommit.playerData[players[i]].commitment.length == 0){
                return false;
            }
        }
        return true;
    }

    function allNizksReady() constant internal returns (bool) {
        for(uint i = 0; i < players.length; i++){
            if(!protocol.stageCommit.playerData[players[i]].initialized ||
            protocol.stageCommit.playerData[players[i]].nizks.length == 0){
                return false;
            }
        }
        return true;
    }

    function allCommitmentsRevealed() constant internal returns (bool) {
        for(uint i = 0; i < players.length; i++){
            bytes memory pubKey = protocol.stageCommit.playerData[players[i]].publicKey;
            if(!protocol.stageCommit.playerData[players[i]].initialized 
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

    function isInTransformationStage() constant internal returns (bool){
        return currentState == State.Stage1 || currentState == State.Stage2 || currentState == State.Stage3;
    }

    function isStringEmpty(string s) pure internal returns (bool) {
        return isBytesEmpty(bytes(s));
    }

    function isBytesEmpty(bytes b) pure internal returns (bool) {
        return b.length == 0;
    }
}