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
    enum State { New, Stage1, Stage2, Stage3 }
    
    /**
     * Events
     */
    event PlayerJoined(address player, string publicKey);
    event StateChanged(uint newState);
    event Committed(address player, string commitment);

    /**
     * State variables
     */
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
        require(!hasCommitmentFor(msg.sender, stateInt-1));
        _;
    }
    modifier stringNotEmpty(string str){
        require(bytes(str).length > 0);
        _;
    }
     
    /**
     * Internal Functions
     */
    function hasCommitmentFor(address player, uint state) internal constant returns(bool) {
        return bytes(playerCommitments[player].commitments[state]).length > 0;
    }

    function goToNextStage() internal {
        currentState = State(uint(currentState) + 1);
        StateChanged(uint(currentState));
    }

    function allCommitmentsReady() internal constant returns(bool){
        if(currentState != State.New){
          uint stateInt = uint(currentState) - 1;
          for(uint index = 0; index < participants.length; index++){
              if(!hasCommitmentFor(participants[index], stateInt)){
                return false;
              }
          }
        }
        return true;
    }

    function isCoordinator() internal constant returns(bool){
        return msg.sender == participants[0];
    }
     
    /**
     * Public Functions
     */
    function DistMpc(){
        currentState = State.New;
        join("coordinator");
    } 
    
    function join(string publicKey) isNewPlayer isInState(State.New) stringNotEmpty(publicKey){
        participants.push(msg.sender);
        playerCommitments[msg.sender] = PlayerCommitment(true, publicKey, new string[](3));
        PlayerJoined(msg.sender, publicKey);
    }
    
    function commit(string commitment) isExistingPlayer isNotInState(State.New) stringNotEmpty(commitment) hasNoCommitmentForCurrentState {
        uint currentStateInt = uint(currentState);
        if(!isCoordinator()){
            require(hasCommitmentFor(participants[0], currentStateInt - 1)); //require that coordinator has committed already
        } else {
            require(!hasCommitmentFor(msg.sender, currentStateInt - 1));
        }
        playerCommitments[msg.sender].commitments[currentStateInt - 1] = commitment;
        Committed(msg.sender, commitment);
        if(allCommitmentsReady()){
            goToNextStage();
        }
    }
    
    function start() isInState(State.New) {
        require(isCoordinator() && participants.length > 1);
        goToNextStage();
    }

}