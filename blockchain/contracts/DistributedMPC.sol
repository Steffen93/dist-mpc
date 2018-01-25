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
        protocol.stageCommit.playerCommitments[msg.sender].commitment = commitment;
        PlayerCommitted(msg.sender, commitment);
        if(allCommitmentsReady()){
            bytes32 hashOfAllCommitments = hashAllCommitments();
            protocol.stageCommit.lastMessageHash = hashOfAllCommitments;
            nextStage();
        }
    }

    function publishPlayerData(string nizks, bytes publicKey)
        public
        isInState(State.Nizks)
        isPlayer
        isNotEmpty(nizks)
        isNotEmptyBytes(publicKey)
        isEmpty(protocol.stageCommit.playerCommitments[msg.sender].nizks)
        isEmptyBytes(protocol.stageCommit.playerCommitments[msg.sender].publicKey)
    {
        require(keccak256(publicKey) == protocol.stageCommit.playerCommitments[msg.sender].commitment);
        protocol.stageCommit.playerCommitments[msg.sender].nizks = nizks;
        protocol.stageCommit.playerCommitments[msg.sender].publicKey = publicKey;
        if(allPlayerDataReady()){
            nextStage();
        }
    }

    function setInitialStage(string stage) 
        public
        isCoordinator
        isInStageTransformationState
    {
        uint stateIndex = uint(currentState) - uint(State.Stage1); // 0 for stage 1, ... 2 for stage 3
        require(isStringEmpty(protocol.initialStages[stateIndex]));
        protocol.initialStages[stateIndex] = stage;
        StagePrepared(uint(currentState));
    }

    function publishStageResults(
        string stageTransformed,
        string iHash
    )
        public
        isInStageTransformationState
        isPlayer
        isNotEmpty(stageTransformed)
        isNotEmpty(iHash)
        previousPlayerCommitted
    {
        uint stateIndex = uint(currentState) - uint(State.Stage1);
        require(isStringEmpty(protocol.stageTransformations[stateIndex].playerCommitments[msg.sender].payload));
        bytes32 lastMessageHash = "";
        if(currentState == State.Stage1){
            bytes storage publicKey = protocol.stageCommit.playerCommitments[msg.sender].publicKey;
            string storage nizks = protocol.stageCommit.playerCommitments[msg.sender].nizks;
            string memory pubkey = string(publicKey);
            lastMessageHash = hashValues(
                pubkey, 
                nizks, 
                stageTransformed, 
                iHash
            );
        } else if (currentState == State.Stage2) {
            //TODO: Check if correct
            lastMessageHash = hashValues(stageTransformed, iHash, "", "");
        } else {
            //TODO: Check if even used and if, how it is calculated in this stage
            lastMessageHash = hashValues(stageTransformed, iHash, "", "");
        }
        protocol.stageTransformations[stateIndex].playerCommitments[msg.sender] = Commitment(stageTransformed, lastMessageHash);
        StageResultPublished(msg.sender, stageTransformed, lastMessageHash);
        if(isLastPlayer()){
            nextStage();
        }
    }
}