pragma solidity ^0.4.11;

import "./MultiPartyProtocol.sol";

contract DistributedMPC is MultiPartyProtocol {
    function DistributedMPC(string r1cs) 
        public
        isNotEmpty(r1cs) 
        MultiPartyProtocol(r1cs) 
    {
        join();
    }
    
    function join() 
        public 
        isInState(State.Join) 
        isNewPlayer 
    {
        players.push(msg.sender);
        PlayerJoined(msg.sender);
    }
    
    function start() 
        public 
        isInState(State.Join) 
        isCoordinator 
        returns (bool)
    {
        return nextStage();
    }

    function commit(bytes32 commitment) 
        public
        isInState(State.Commit) 
        isPlayer 
        isNotEmptyBytes32(commitment)
        isEmptyBytes32(protocol.stageCommit.playerCommitments[msg.sender].commitment)
    {
        protocol.stageCommit.playerCommitments[msg.sender].initialized = true;
        protocol.stageCommit.playerCommitments[msg.sender].commitment = commitment;
        protocol.stageCommit.commitmentLength += commitment.length;
        PlayerCommitted(msg.sender, commitment);
        if(allCommitmentsReady()){
            protocol.stageCommit.hashOfAllCommitments = hashAllCommitments();
            nextStage();
        }
    }

    function getHashOfAllCommitments() public constant returns(bytes32){
        return protocol.stageCommit.hashOfAllCommitments;
    }

    function publishPlayerData(bytes nizks, bytes publicKey)
        public
        isInState(State.Nizks)
        isPlayer
        isNotEmptyBytes(nizks)
        isNotEmptyBytes(publicKey)
        isEmptyBytes(protocol.stageCommit.playerCommitments[msg.sender].nizks)
        isEmptyBytes(protocol.stageCommit.playerCommitments[msg.sender].publicKey)
    {
        require(keccak256(publicKey) == protocol.stageCommit.playerCommitments[msg.sender].commitment);
        require(publicKey.length == 2069);
        protocol.stageCommit.playerCommitments[msg.sender].nizks = nizks;
        protocol.stageCommit.playerCommitments[msg.sender].publicKey = publicKey;
        if(allPlayerDataReady()){
            nextStage();
        }
    }

    function setInitialStage(bytes stage) 
        public
        isCoordinator
        isInStageTransformationState
    {
        uint stateIndex = uint(currentState) - uint(State.Stage1); // 0 for stage 1, ... 2 for stage 3
        require(isBytesEmpty(protocol.initialStages[stateIndex]));
        protocol.initialStages[stateIndex] = stage;
        StagePrepared(uint(currentState));
    }

    function publishStageResults(
        bytes stageTransformed
    )
        public
        isInStageTransformationState
        isPlayer
        isNotEmptyBytes(stageTransformed)
        previousPlayerCommitted
    {
        uint stateIndex = uint(currentState) - uint(State.Stage1);
        require(isBytesEmpty(protocol.stageTransformations[stateIndex].playerCommitments[msg.sender]));
        if(currentState == State.Stage1){
            // bytes storage publicKey = protocol.stageCommit.playerCommitments[msg.sender].publicKey;
            // bytes storage nizks = protocol.stageCommit.playerCommitments[msg.sender].nizks;
        } else {
            // TODO: handle
        }
        protocol.stageTransformations[stateIndex].playerCommitments[msg.sender] = stageTransformed;
        StageResultPublished(msg.sender, stageTransformed);
        if(isLastPlayer()){
            nextStage();
        }
    }
}