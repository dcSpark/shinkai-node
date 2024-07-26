#!/bin/bash

cargo build

for i in {1..1000}
do
  ./target/debug/shinkai_subscription_management_cli subscribe_to_folder /new_hope @@external_identity_testing_tcp_relay.arb-sep-shinkai main --http-preferred true
done
