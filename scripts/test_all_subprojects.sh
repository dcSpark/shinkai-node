set -e
export CC=gcc
export CXX=g++

clear
cargo test -- --test-threads=1
cd shinkai-message-wasm
wasm-pack build
cp pkg ../shinkai-app -r
wasm-pack test --node
cargo test -- --test-threads=1
cd ..
cd shinkai-app
npm install
npm run test.unit
