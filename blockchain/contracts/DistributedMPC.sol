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
        isSenderPlayer 
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
        isSenderPlayer
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
        isSenderPlayer
        isNotEmptyBytes(nizks)
        isEmptyBytes(protocol.stageCommit.playerData[msg.sender].nizks)
    {                                   
        require(nizks.length == 1032);
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
        isSenderPlayer
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

    function getConstraintSystem()
        constant
        public
        returns (bytes)
    {
        return protocol.r1cs;
    }

    function getInitialStage(uint stage)
        constant
        public
        returns (bytes) 
    {
        require(stage < protocol.stageTransformations.length);
        return protocol.initialStages[stage];
    }

    function getTransformation(uint stage, uint playerIndex)
        constant
        public
        returns (bytes)
    {
        require(stage < protocol.stageTransformations.length);
        require(playerIndex < players.length);
        return protocol.stageTransformations[stage].playerData[players[playerIndex]];
    }

    function getLatestTransformation() 
        constant 
        public 
        returns (bytes) 
    {
        return protocol.latestTransformation;
    }

    function getNizks(uint playerIndex)
        constant
        public
        returns (bytes)
    {
        require(playerIndex < players.length);
        return protocol.stageCommit.playerData[players[playerIndex]].nizks;
    }

    function getNumberOfPlayers()
        constant
        public
        returns (uint)
    {
        return players.length;
    }

    function getPublicKey(uint playerIndex)
        constant
        public
        returns (bytes)
    {
        require(playerIndex < players.length);
        return protocol.stageCommit.playerData[players[playerIndex]].publicKey;
    }

    function isCoordinator()
        constant
        public
        returns (bool) 
    {
        return msg.sender == players[0];
    }

    function isPlayer()
        constant
        public
        returns (bool)
    {
        for(uint i = 0; i < players.length; i++){
            if(players[i] == msg.sender){
                return true;
            }
        }
        return false;
    }
}