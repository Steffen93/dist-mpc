pragma solidity ^0.4.0;

import "./MultiPartyProtocol.sol";

contract DistributedMPC is MultiPartyProtocol {
    function DistributedMPC(string r1cs) isNotEmpty(r1cs) MultiPartyProtocol(r1cs) {
        join();
    }
    
    function join() isInState(State.Join) isNewPlayer {
        players.push(msg.sender);
        PlayerJoined(msg.sender);
    }
    
    function start() isInState(State.Join) isCoordinator returns (bool){
        return nextStage();
    }

    function commit(string commitment) 
        isInState(State.Commit) 
        isPlayer 
        isNotEmpty(commitment)
        isEmpty(protocol.stageCommit.playerCommitments[msg.sender].commitment)
    {
        protocol.stageCommit.playerCommitments[msg.sender].commitment = commitment;
        PlayerCommitted(msg.sender, commitment);
        if(allCommitmentsReady()){
            bytes32 hashOfAllCommitments = hashAllCommitments();
            protocol.stageCommit.lastMessageHash = hashOfAllCommitments;
            nextStage();
        }
    }

    function publishPlayerData(string nizks, string publicKey)
        isInState(State.Nizks)
        isPlayer
        isNotEmpty(nizks)
        isNotEmpty(publicKey)
        isEmpty(protocol.stageCommit.playerCommitments[msg.sender].nizks)
        isEmpty(protocol.stageCommit.playerCommitments[msg.sender].publicKey)
    {
        require(sha3(publicKey) == stringToBytes32(protocol.stageCommit.playerCommitments[msg.sender].commitment));
        protocol.stageCommit.playerCommitments[msg.sender].nizks = nizks;
        protocol.stageCommit.playerCommitments[msg.sender].publicKey = publicKey;
        if(allPlayerDataReady()){
            nextStage();
        }
    }

    function setInitialStage(string stage) 
        isCoordinator
    {
        require(
            currentState == State.Stage1 
            || currentState == State.Stage2 
            || currentState == State.Stage3
        );
        uint stateInt = uint(currentState) - uint(State.Stage1); // 0 for stage 1, ... 2 for stage 3
        require(isStringEmpty(protocol.initialStages[stateInt]));
        protocol.initialStages[stateInt] = stage;
        StagePrepared(uint(currentState));
    }

    function publishStageOneResults(
        string stageOneTransformed,
        string iHash
    )
        isInState(State.Stage1)
        isPlayer
        isNotEmpty(stageOneTransformed)
        isNotEmpty(iHash)
        isEmpty(protocol.stageTransformations[0].playerCommitments[msg.sender].payload)
        previousPlayerCommitted
    {
        //TODO: check that previous player has committed
        /* FIXME: adapt to changes
        bytes32 lastMessageHash = hashStageOneResults(
            publicKey, 
            nizks, 
            stageOneTransformed, 
            iHash
        );
        protocol.stage1.playerCommitments[msg.sender] = PlayerStage1(nizks, publicKey, Commitment(stageOneTransformed, lastMessageHash));
        */
    }
}