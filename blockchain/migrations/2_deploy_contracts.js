var DistributedMPC = artifacts.require("./DistributedMPC.sol");

module.exports = function(deployer) {
  deployer.deploy(DistributedMPC);
};
