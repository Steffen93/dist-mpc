var DistributedMPC = artifacts.require("./DistributedMPC.sol");
var fs = require('fs');

var r1cs = fs.readFileSync('../r1cs', {
    encoding: 'utf8'
}).toString();
console.log('r1cs: %s', r1cs);

module.exports = function(deployer) {
  deployer.deploy(DistributedMPC, r1cs);
};
