# Dist-mpc
Distributed variant of the Zcash [multi-party computation protocol](https://github.com/zcash/mpc).
We use the Ethereum blockchain to distribute the key generation process and take those parameters from there for generating zk-SNARKs.

The following steps are required to run the setup:

1. Deploy the DistMpc contract on the ethereum blockchain. See details in the according [README](blockchain/README.md).

2. Run the multi-party protocol. See details in the according [README](mpc/README.md).