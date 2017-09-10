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
        isEmpty(protocol.stage0.playerCommitments[msg.sender])
    {
        protocol.stage0.playerCommitments[msg.sender] = commitment;
        PlayerCommitted(msg.sender, commitment);
        if(allCommitmentsReady()){
            bytes32 hashOfAllCommitments = hashAllCommitments();
            protocol.stage0.lastMessageHash = hashOfAllCommitments;
            nextStage();
        }
    }

    function setInitialStage(string stage) isCoordinator {
        uint stateInt = uint(currentState) - 2;
        require(stateInt >= 0 && stateInt <= 2); //only possible in state Stage1, Stage2 or Stage3
        require(isStringEmpty(protocol.initialStages[stateInt]));
        protocol.initialStages[stateInt] = stage;
        StagePrepared(uint(currentState));
    }

    function publishStageOneResults(
        string nizks, 
        string publicKey, 
        string stageOneTransformed,
        string iHash
    )
        isInState(State.Stage1)
        isPlayer
        isNotEmpty(nizks)
        isNotEmpty(publicKey)
        isNotEmpty(stageOneTransformed)
        isNotEmpty(iHash)
    {
        //TODO: check that previous player has committed
        require(sha3(publicKey) == stringToBytes32(protocol.stage0.playerCommitments[msg.sender]));
        bytes32 lastMessageHash = hashStageOneResults(
            publicKey, 
            nizks, 
            stageOneTransformed, 
            iHash
        );
        protocol.stage1.playerCommitments[msg.sender] = PlayerStage1(nizks, publicKey, Commitment(stageOneTransformed, lastMessageHash));
    }
}