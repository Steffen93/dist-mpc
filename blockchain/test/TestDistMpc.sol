pragma solidity ^0.4.2;

import "truffle/Assert.sol";
import "truffle/DeployedAddresses.sol";
import "../contracts/DistMpc.sol";

contract TestDistMpc {

  function testInitialContractDeployment() {
    DistMpc dmpc = new DistMpc();
    //Assert.equal(dmpc.participants.length, 0, "Contract should not have participants");
  }
}
