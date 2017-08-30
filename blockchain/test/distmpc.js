var DistMpc = artifacts.require("./DistMpc.sol");

contract('DistMpc', function(accounts) {
  it("should set the first account as coordinator", function() {
    return DistMpc.deployed().then(function(instance) {
      return instance.coordinator();
    }).then(function(coordinator) {
      assert.equal(coordinator, accounts[0], "The default account is not the coordinator.");
    }).catch(function(error){
      assert.fail("Something went wrong.")
    });
  });

  it("should join successfully", function() {
    return DistMpc.deployed().then(function(instance) {
      return instance.join("MyPubKey",{from: accounts[1]});
    }).then(function() {
      assert.ok(true);
    }).catch(function(error){
      assert.fail("Joining failed.");
    });
  });
  
  it("should fail to join twice", function(){
    return DistMpc.deployed().then(function(instance) {
      return instance.join("SomeOtherPubKey", {from: accounts[1]});
    }).then(function(){
      assert.fail("It should fail to join twice");
    }).catch(function(){
      assert.ok(true);
    });
  });

  it("should fail to commit in stage 'New'", function(){
    return DistMpc.deployed().then(function(instance) {
      return instance.commit("My first commitment", {from: accounts[1]});
    }).then(function(){
      assert.fail("Commitment should have failed.");
    }).catch(function(){
      assert.ok(true);
    });
  });

  it("should fail to go to next stage if not coordinator", function(){
    return DistMpc.deployed().then(function(instance) {
      return instance.goToNextStage({from: accounts[1]});
    }).then(function(){
      assert.fail("This should not work.");
    }).catch(function(){
      assert.ok(true);
    });
  });

  it("should succeed to go to next stage if sender is coordinator", function(){
    return DistMpc.deployed().then(function(instance) {
      return instance.goToNextStage({from: accounts[0]});
    }).then(function(){
      assert.ok(true);
    }).catch(function(){
      assert.fail("Coordinator should be able to go to next stage.");
    });
  });

  it("should fail to go to next stage if not all players committed yet", function(){
    return DistMpc.deployed().then(function(instance) {
      return instance.goToNextStage({from: accounts[0]});
    }).then(function(){
      assert.fail("Should fail. Not all players committed yet.");
    }).catch(function(){
      assert.ok(true);
    });
  });

  it("should fail to commit if sender is not a participant", function(){
    var dmpc;
    return DistMpc.deployed().then(function(instance) {
      dmpc = instance;
      return dmpc.commit("commitment 1", {from: accounts[2]});
    }).then(function(){
      assert.fail("Sender is not a participant.");
    }).catch(function(){
      assert.ok(true);
    });
  });

  it("should commit successfully to the first stage", function(){
    var dmpc;
    return DistMpc.deployed().then(function(instance) {
      dmpc = instance;
      return dmpc.commit("commitment 1", {from: accounts[1]});
    }).then(function(){
      assert.ok(true);
    }).catch(function(){
      assert.fail("Commitment should have been successful.");
    });
  });

  it("should work from first to last stage", function(){
    var dmpc;
    return DistMpc.new().then(function(instance){
      dmpc = instance;
      return dmpc.join("MyPubKey", {from: accounts[1]});
    }).then(function(){
      return dmpc.goToNextStage({from: accounts[0]});
    }).then(function(){
      return dmpc.commit("Stage1Commitment", {from: accounts[1]});
    }).then(function(){
      return dmpc.goToNextStage({from: accounts[0]});
    }).then(function(){
      return dmpc.commit("Stage2Commitment", {from: accounts[1]});
    }).then(function(){
      return dmpc.goToNextStage({from: accounts[0]});
    }).then(function(){
      return dmpc.commit("Stage3Commitment", {from: accounts[1]});
    }).then(function(){
      return dmpc.goToNextStage({from: accounts[0]});
    }).then(function(){
      return dmpc.commit("Stage4Commitment", {from: accounts[1]});
    }).then(function(){
      return dmpc.goToNextStage({from: accounts[0]});
    }).then(function(){
      return dmpc.commit("Stage5Commitment", {from: accounts[1]});
    }).then(function(){
      return dmpc.goToNextStage({from: accounts[0]});
    }).then(function(){
      return dmpc.commit("Stage6Commitment", {from: accounts[1]});
    }).then(function(){
      assert.ok(true);
    }).catch(function(error){
      assert.fail("Should not fail during the process.");
    });
  });
  
});
