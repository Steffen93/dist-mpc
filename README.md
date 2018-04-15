# Dist-mpc

**Please note that this software is a prototype and not ready for production!**

Distributed variant of the Zcash [multi-party computation protocol](https://github.com/zcash/mpc).
We use the Ethereum blockchain to distribute the key generation process and take those parameters from there for generating zk-SNARKs.

The following steps are required to run the setup:

1. Deploy the DistMpc contract on the ethereum blockchain. See details in the according [README](blockchain).

2. Run the multi-party protocol. See details in the according [README](mpc).

Alternatively, the project can be run in a Docker container.

## Compatibility with ZoKrates
dist-mpc aims to become compatible with the [ZoKrates toolset](https://github.com/JacobEberhardt/ZoKrates).
Because of an incompatibility of the constraint systems ZoKrates produces and this tool expects, currently [this fork](https://github.com/steffen93/ZoKrates) needs to be used to generate a valid R1CS.

However, the proving key and verification key are currently not compatible with ZoKrates, which needs to be resolved.

-----
# Note for developers
On contract change, the ABI needs to be copied to the mpc folder into the `abi.json` file.
That file is required if the contract is loaded from an existing address. Only if the contract is deployed in this session, the player executable reads the abi from the blockchain folder in the truffle build directory.
