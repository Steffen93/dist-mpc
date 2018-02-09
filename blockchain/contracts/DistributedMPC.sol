pragma solidity ^0.4.11;

import "./MultiPartyProtocol.sol";

contract DistributedMPC is MultiPartyProtocol {
    function DistributedMPC(bytes r1cs) 
        public
        isNotEmptyBytes(r1cs) 
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
        isSenderCoordinator 
        returns (bool)
    {
        return nextStage();
    }

    function commit(bytes32 commitment) 
        public
        isInState(State.Commit) 
        isPlayer 
        isNotEmptyBytes32(commitment)
        isEmptyBytes32(protocol.stageCommit.playerData[msg.sender].commitment)
    {
        protocol.stageCommit.playerData[msg.sender].initialized = true;
        protocol.stageCommit.playerData[msg.sender].commitment = commitment;
        PlayerCommitted(msg.sender, commitment);
        if(allCommitmentsReady()){
            nextStage();
        }
    }
    
    function revealCommitment(bytes publicKey)
        public
        isInState(State.Reveal)
        isPlayer
        isNotEmptyBytes(publicKey)
        isEmptyBytes(protocol.stageCommit.playerData[msg.sender].publicKey)
    {
        require(publicKey.length == 2069);
        require(keccak256(publicKey) == protocol.stageCommit.playerData[msg.sender].commitment);
        protocol.stageCommit.playerData[msg.sender].publicKey = publicKey;
        if(allCommitmentsRevealed()){
            nextStage();
        }
    }

    function publishNizks(bytes nizks)
        public
        isInState(State.Nizks)
        isPlayer
        isNotEmptyBytes(nizks)
        isEmptyBytes(protocol.stageCommit.playerData[msg.sender].nizks)
    {                                   
        protocol.stageCommit.playerData[msg.sender].nizks = nizks;
        if(allNizksReady()){
            nextStage();
        }
    }

    function setInitialStage(bytes stage) 
        public
        isSenderCoordinator
        isInStageTransformationState
    {
        uint stateIndex = uint(currentState) - uint(State.Stage1); // 0 for stage 1, ... 2 for stage 3
        require(isBytesEmpty(protocol.initialStages[stateIndex]));
        protocol.initialStages[stateIndex] = stage;
        protocol.latestTransformation = stage;
        StagePrepared(uint(currentState));
    }

    function publishStageResults(bytes stageTransformed)
        public
        isInStageTransformationState
        isPlayer
        isNotEmptyBytes(stageTransformed)
        previousPlayerCommitted
    {
        uint stateIndex = uint(currentState) - uint(State.Stage1);
        require(isBytesEmpty(protocol.stageTransformations[stateIndex].playerData[msg.sender]));
        protocol.stageTransformations[stateIndex].playerData[msg.sender] = stageTransformed;
        protocol.latestTransformation = stageTransformed;
        StageResultPublished(msg.sender, stageTransformed);
        if(isLastPlayer()){
            nextStage();
        }
    }

    function getCommitment(address player)
        constant
        public
        returns (bytes32)
    {
        return protocol.stageCommit.playerData[player].commitment;
    }

    function getInitialStage()
        constant
        public
        isInStageTransformationState
        isPlayer
        returns (bytes) 
    {
        uint stateIndex = uint(currentState) - uint(State.Stage1);
        return protocol.initialStages[stateIndex];
    }

    function getLatestTransformation() 
        constant 
        public 
        isInStageTransformationState
        isPlayer
        returns (bytes) 
    {
        return protocol.latestTransformation;
    }

    function getNumberOfPlayers()
        constant
        public
        returns (uint)
    {
        return players.length;
    }

    function isCoordinator()
        constant
        public
        returns (bool) 
    {
        return msg.sender == players[0];
    }
}