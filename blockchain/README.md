# How to run the blockchain part

If not installed yet, please install truffle:
```
npm install -g truffle
```

If you want to test the implementation with a local ethereum network, you can install testrpc:
```
npm install -g ethereumjs-testrpc
```

## Migrate
First make sure that an ethereum network is running truffle can deploy the contract to.
In case of testrpc, run in a new terminal window:
```
testrpc
```
Now, in a separate window, run:
```
truffle migrate
```
This should deploy the contract on the ethereum network.

## Unit tests
You can run the tests with the following command:
```
truffle test
```