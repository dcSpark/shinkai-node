## TODO (non-exhaustive)

- [ ] Add a check that after you register for the first time with a profile, the device checks on-chain that the information is correct and it's not spoofed.
- [ ] If a device from a profile sends a message to the node and it fails because of wrong encryption keys for the node, the error message should be clear for the device so then it can check the on-chain identities and update the public keys for the node if needed (it shouldn't ask the node directly for the new keys!).