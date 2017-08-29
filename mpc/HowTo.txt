Run the following commands in a terminal each:
cargo run --bin coordinator                     //hosts TCP Connection
cargo run --bin compute --no-default-features   //Generate first commitment
cargo run --bin network --no-default-features   //Connect to coordinator

1. Compute -> enter random seed -> hash (qTTQwHCF6SZDWPxMNapTZds91aH4xqPwNkCXkL8XyTNowYuGY)
2. Network -> enter hash from step 1
3. Coordinator coordinates
4. Network -> insert empty DVD to burn it ("A") (discA)
5. Compute -> read disc A -> hash (25sgF8k3kjSjUpAFFyisJyNUt1B8T8JrwquTUAy72FH58jSPa3)
6. Compute -> write disc B -> hash (TvHWtLZXMXUzC9QpAStbbgHGE56LHHzKEnJW4rNHv7nMFJEeo)
7. Network -> read disc B -> and so on
...

In the end you have the complete transcript file in the base directory.

You can verify the transcript and generate the public parameters using the following command: 
cargo run --bin verifier

This will generate two files: "pk" (proving key) and "vk" (verification key)
