var DistMpc = artifacts.require("./DistributedMPC.sol");
var assert = require("assert");

let expectFailHandler = (promise, message) => {
  return promise.then(() => {
    assert.fail(message);
  }).catch(_ => {
    assert.ok(true);
  });
}

let expectSuccessHandler = (promise, message) => {
  return promise.then(() => {
    assert.ok(true);
  }).catch(_ => {
    assert.fail(message);
  });
}

let expectEqual = (promise, expectedValue, message) => {
  return promise.then(value => {
    assert.equal(value, expectedValue, message);
  }).catch(error => {
    assert.fail("Something went wrong");
  });
}

let getHash = (value) => {
  return "0x" + web3.sha3(value);
}

contract('DistributedMPC', accounts => {

  /***********************************************/
  /************** Stage: Join ********************/
  /***********************************************/

  describe('Stage: Join', () => {
    it("should set the first account as coordinator", () =>  {
      let p = DistMpc.deployed().then(instance => {
        return instance.players(0);
      });
      return expectEqual(p, accounts[0], "The default account is not the coordinator.");
    });

    it("should join successfully", () =>  {
      let p = DistMpc.deployed().then(instance => {
        return instance.join({from: accounts[1]});
      });
      return expectSuccessHandler(p, "Joining should have been successful");
    });
    
    it("should fail to join twice", () => {
      let p = DistMpc.deployed().then(instance => {
        return instance.join({from: accounts[1]});
      });
      return expectFailHandler(p, "It should fail to join twice");
    });

    it("should fail to commit in stage 'Join'", () => {
      let p = DistMpc.deployed().then(instance => {
        return instance.commit("My first commitment", {from: accounts[1]});
      });
      return expectFailHandler(p, "Commitment should have failed.");
    });

    it("should fail to start if not coordinator", () => {
      let p = DistMpc.deployed().then(instance => {
        return instance.start({from: accounts[1]});
      });
      return expectFailHandler(p, "Starting should not work if not coordinator.");
    });

    it("should start if sender is coordinator", () => {
      let p = DistMpc.deployed().then(instance => {
        return instance.start({from: accounts[0]});
      });
      return expectSuccessHandler(p, "Coordinator should be able to go to next stage.");
    });
  });

  /***********************************************/
  /************** Stage: Commit ******************/
  /***********************************************/

  describe('Stage: Commit', () => {
    it("should be stage 1 at this point", () => {
      let p = DistMpc.deployed().then(instance => {
        return instance.currentState();
      });
      return expectEqual(p, 1, "Should be in Stage 1");
    });
    it("should be impossible to join anymore", () => {
      let p = DistMpc.deployed().then(instance => {
        return instance.join({from: accounts[2]});
      });
      return expectFailHandler(p, "Should not be able to join after join phase");
    });
    it("should fail to commit if sender is not a player", () => {
      let p = DistMpc.deployed().then(instance => {
        return instance.commit("commitment 1", {from: accounts[2]});
      });
      return expectFailHandler(p, "Sender is not a player.");
    });
    it("should fail to commit if commitment is empty", () => {
      let p = DistMpc.deployed().then(instance => {
        return instance.commit("", {from: accounts[1]});
      });
      return expectFailHandler(p, "This should not succeed.");
    });

    it("should commit successfully to the first stage", () => {
      let p = DistMpc.deployed().then(instance => {
        let commitments = [];
        commitments.push(instance.commit(getHash("commitment1"), {from: accounts[0]}));
        commitments.push(instance.commit(getHash("commitment2"), {from: accounts[1]}));
        return Promise.all(commitments);
      });
      return expectSuccessHandler(p, "Commitments should've been successful.");
    });

    it("should have proceeded to next stage when everyone has committed", () => {
      let p = DistMpc.deployed().then(instance => {
        return instance.currentState();
      });
      return expectEqual(p, 2, "Should be in Stage 2");
    });

    it("should fail to commit again after successful commitment", () => {
      let p = DistMpc.deployed().then(instance => {
        return instance.commit("commitment 1", {from: accounts[1]});
      });
      return expectFailHandler(p, "Should not be able to commit twice");
    });
  });

  /***********************************************/
  /************** Stage: Nizks  ******************/
  /***********************************************/

  describe('Stage: Nizks', () => {
    it("should fail to publish data if not a player", () => {
      let p = DistMpc.deployed().then(instance => {
        return instance.publishPlayerData("something", getHash("commitment1"), {from: accounts[2]});
      });
      return expectFailHandler(p, "Should fail to commit if not a player");
    });
    
    it("should fail to publish data if nizks is empty", () => {
      let p = DistMpc.deployed().then(instance => {
        return instance.publishPlayerData("", getHash("commitment1"), {from: accounts[0]});
      });
      return expectFailHandler(p, "Should fail because nizks was empty");
    });
    
    it("should fail to publish data if publicKey is empty", () => {
      let p = DistMpc.deployed().then(instance => {
        let p = DistMpc.deployed().then(instance => {
          return instance.publishPlayerData("something", "", {from: accounts[0]});
        });
        return expectFailHandler(p, "Should fail because publicKey was empty");
      });
    });
    
    it("should fail to publish data if publicKey != hash of commitment", () => {
      let p = DistMpc.deployed().then(instance => {
        let p = DistMpc.deployed().then(instance => {
          return instance.publishPlayerData("something", "notahash", {from: accounts[0]});
        });
        return expectFailHandler(p, "Should fail because publicKey was not hash of commitment");
      });
    });

    it("should succeed to publish data if publicKey is valid and nizks is given", () => {
      let p = DistMpc.deployed().then(instance => {
        let commitments = [];
        commitments.push(instance.publishPlayerData("something", "commitment1", {from: accounts[0]}));
        commitments.push(instance.publishPlayerData("something", "commitment2", {from: accounts[1]}));
        return Promise.all(commitments);
      });

      return expectSuccessHandler(p, "Should have succeeded to publish data.");
    });

    it("should have proceeded to next stage when everyone has committed", () => {
      let p = DistMpc.deployed().then(instance => {
        return instance.currentState();
      });
      return expectEqual(p, 3, "Should be in Stage 3");
    });

  });
});
