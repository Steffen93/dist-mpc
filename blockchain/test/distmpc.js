var DistMpc = artifacts.require("./DistMpc.sol");

contract('DistMpc', function(accounts) {
  it("should set the first account as coordinator", function() {
    return DistMpc.deployed().then(function(instance) {
      return instance.participants(0);
    }).then(function(coordinator) {
      assert.equal(coordinator, accounts[0], "The default account is not the coordinator.");
    }).catch(function(error){
      assert.fail("Something went wrong.")
    });
  });

  it("should fail to start without participants", function(){
    return DistMpc.deployed().then(function(instance) {
      return instance.start({from: accounts[0]});
    }).then(function(){
      assert.fail("It should fail to start");
    }).catch(function(){
      assert.ok(true);
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

  it("should fail to go start if not coordinator", function(){
    return DistMpc.deployed().then(function(instance) {
      return instance.start({from: accounts[1]});
    }).then(function(){
      assert.fail("This should not work.");
    }).catch(function(){
      assert.ok(true);
    });
  });

  it("should go to next stage if sender is coordinator", function(){
    return DistMpc.deployed().then(function(instance) {
      return instance.start({from: accounts[0]});
    }).then(function(){
      assert.ok(true);
    }).catch(function(){
      assert.fail("Coordinator should be able to go to next stage.");
    });
  });

  it("should fail to commit if sender is not a participant", function(){
    return DistMpc.deployed().then(function(instance) {
      return instance.commit("commitment 1", {from: accounts[2]});
    }).then(function(){
      assert.fail("Sender is not a participant.");
    }).catch(function(){
      assert.ok(true);
    });
  });

  it("should fail to commit if sender is a participant but coordinator has not committed yet", function(){
    return DistMpc.deployed().then(function(instance) {
      return instance.commit("commitment 1", {from: accounts[1]});
    }).then(function(){
      assert.fail("Coordinator has not yet committed.");
    }).catch(function(){
      assert.ok(true);
    });
  });

  it("should commit successfully to the first stage if sender is coordinator", function(){
    return DistMpc.deployed().then(function(instance) {
      return instance.commit("commitment 1", {from: accounts[0]});
    }).then(function(){
      assert.ok(true);
    }).catch(function(){
      assert.fail("Commitment should have been successful.");
    });
  });

  it("should commit successfully to the first stage", function(){
    return DistMpc.deployed().then(function(instance) {
      return instance.commit("commitment 1", {from: accounts[1]});
    }).then(function(){
      assert.ok(true);
    }).catch(function(){
      assert.fail("Commitment should have been successful.");
    });
  });
/* FIXME: Adapt to correct structure
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
*/
});
