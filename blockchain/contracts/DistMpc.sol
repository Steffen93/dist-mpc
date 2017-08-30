pragma solidity ^0.4.0;

contract DistMpc {
    /**
     * Structs
     */
    // Commitments made by a player during the setup phase
    struct PlayerCommitment {
        bool initialized;
        string publicKey;
        string[] commitments;
    }
    
    /**
     * Enums
     */
    enum State { New, StageA, StageB, StageC, StageD, StageE, StageF }
    
    /**
     * State variables
     */
    address public coordinator;
    address[] public participants; // array of all participants
    mapping(address => PlayerCommitment) playerCommitments; // map a player to its commitments
    State currentState;
    
    /**
     * Function modifiers
     */
    modifier isNewPlayer(){
        require(!playerCommitments[msg.sender].initialized);
        _;
    }
    modifier isExistingPlayer() {
        require(playerCommitments[msg.sender].initialized);
        _;
    }
    modifier isInState(State state){
        require(currentState == state);
        _;
    }
    modifier isNotInState(State state){
        require(currentState != state);
        _;
    }
    modifier hasNoCommitmentForCurrentState(){
        uint stateInt = uint(currentState);
        require(!hasCommitmentFor(stateInt-1));
        _;
    }
    modifier isCoordinator(){
        require(msg.sender == coordinator);
        _;
    }
    modifier allCommitmentsReady(){
        if(currentState != State.New){
          uint stateInt = uint(currentState) - 1;
          for(uint partCount = 0; partCount < participants.length - 1; partCount++){
              require(hasCommitmentFor(stateInt));
          }
        }
        _;
    }
     
    /**
     * Internal Functions
     */
    function hasCommitmentFor(uint state) internal returns (bool) {
        return bytes(playerCommitments[msg.sender].commitments[state]).length > 0;
    }
     
    /**
     * Public Functions
     */
    function DistMpc(){
        coordinator = msg.sender;
        currentState = State.New;
    }
    
    function join(string publicKey) isNewPlayer isInState(State.New) {
        participants.push(msg.sender);
        playerCommitments[msg.sender] = PlayerCommitment(true, publicKey, new string[](6));
    }
    
    function commit(string commitment) isExistingPlayer isNotInState(State.New) hasNoCommitmentForCurrentState {
        uint currentStateInt = uint(currentState);
        playerCommitments[msg.sender].commitments[currentStateInt - 1] = commitment;
    }
    
    function goToNextStage() isCoordinator allCommitmentsReady isNotInState(State.StageF) {
        require(participants.length > 0);
        currentState = State(uint(currentState) + 1);
    }
}