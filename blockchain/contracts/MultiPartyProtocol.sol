pragma solidity ^0.4.11;

import "./strings.sol";

contract MultiPartyProtocol {
    using strings for *;
    struct Commitment {
        string payload;
        bytes32 lastMessageHash;
    }

    struct PlayerCommitment {
        bytes32 commitment;
        string nizks;
        bytes publicKey;
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
    
    event PlayerJoined(address player);    //called when a player joined the protocol
    event PlayerCommitted(address player, bytes32 commitment); //called when a player committed hash of public key
    event NextStage(uint stage);  //called when a new stage begins
    event StagePrepared(uint stage);  //called when the coordinator initialized a new stage (stage1, stage2, stage3)
    event StageResultPublished(address player, string result, bytes32 hash);
    
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
    
    function MultiPartyProtocol(string r1cs) public {
        protocol.r1cs = r1cs;
        protocol.initialStages = new string[](3);
        protocol.stageCommit = StageCommit("");
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
            if(protocol.stageCommit.playerCommitments[players[i]].commitment.length == 0){
                return false;
            }
        }
        return true;
    }

    function allPlayerDataReady() constant internal returns (bool) {
        for(uint i = 0; i < players.length; i++){
            string memory nizks = protocol.stageCommit.playerCommitments[players[i]].nizks;
            bytes memory pubKey = protocol.stageCommit.playerCommitments[players[i]].publicKey;
            if(isStringEmpty(nizks) || isBytesEmpty(pubKey)){
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

    function hashAllCommitments() constant internal returns (bytes32) {
        string memory allCommitments = "";
        for(uint i; i < players.length; i++){
            allCommitments = allCommitments.toSlice().concat(
                bytes32ToString(protocol.stageCommit.playerCommitments[players[i]].commitment).toSlice()
            );
        }
        return keccak256(allCommitments);
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
        return keccak256(
            pubKey.toSlice()
            .concat(nizks.toSlice()).toSlice()
            .concat(s1Transformed.toSlice()).toSlice()
            .concat(iHash.toSlice())
        );
    }

    function hashValues(
        string str1, 
        string str2, 
        string str3, 
        string str4
    ) 
        constant 
        internal 
        returns (bytes32) 
    {
        return keccak256(
            str1.toSlice()
            .concat(str2.toSlice()).toSlice()
            .concat(str3.toSlice()).toSlice()
            .concat(str4.toSlice())
        );
    }

    function isStringEmpty(string s) pure internal returns (bool) {
        return isBytesEmpty(bytes(s));
    }

    function isBytesEmpty(bytes b) pure internal returns (bool) {
        return b.length == 0;
    }

    function stringToBytes32(string memory source) pure internal returns (bytes32 result) {
        assembly {
            result := mload(add(source, 32))
        }
    }

    function bytes32ToString(bytes32 x) pure internal returns (string) {
        bytes memory bytesString = new bytes(32);
        uint charCount = 0;
        for (uint j = 0; j < 32; j++) {
            byte char = byte(bytes32(uint(x) * 2 ** (8 * j)));
            if (char != 0) {
                bytesString[charCount] = char;
                charCount++;
            }
        }
        bytes memory bytesStringTrimmed = new bytes(charCount);
        for (j = 0; j < charCount; j++) {
            bytesStringTrimmed[j] = bytesString[j];
        }
        return string(bytesStringTrimmed);
    }
}