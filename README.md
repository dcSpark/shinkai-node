<h1 align="center">
  <img src="files/icon.png"/><br/>
  Shinkai Node
</h1>
<p align="center">The Shinkai Node is the central unit within the Shinkai Network that links user devices and oversees AI agents. Its diverse functions include processing user inputs, managing AI models, handling external containerized tooling for AI, coordinating computing tasks, generating proofs, converting and indexing data into vector embeddings, and ensuring efficient task execution according to user needs. The nodes play a crucial role in maintaining the network's decentralized structure, which enhances both security and privacy.<br/><br/> There is a companion repo called Shinkai Apps, that allows you to locally run the node and also easily manage AI models using Ollama, you can find it <a href="https://github.com/dcSpark/shinkai-apps">here</a>.</p><br/>

[![Mutable.ai Auto Wiki](https://img.shields.io/badge/Auto_Wiki-Mutable.ai-blue)](https://wiki.mutable.ai/dcSpark/shinkai-node)

## Documentation

General Documentation: [https://docs.shinkai.com](https://docs.shinkai.com)

More In Depth Codebase Documentation (Mutable.ai): [https://wiki.mutable.ai/dcSpark/shinkai-node](https://wiki.mutable.ai/dcSpark/shinkai-node)

## Quick Install

First install and run [ollama](https://ollama.com/download) 

### Linux and MacOS

You can **install** and **run** shinkai-node by running this command in your terminal

```sh
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/main/scripts/download.sh | bash
```

### Windows
Download and run the [shinkai-node binaries](https://github.com/dcSpark/shinkai-node/releases/latest/download/shinkai-node-x86_64-pc-windows-msvc.zip)

## Build 

### Quick build
```sh
git clone https://github.com/dcSpark/shinkai-node
cd shinkai-node
sh scripts/run_node_localhost.sh
```

For complete instructions on compiling and testing go to [compiling.md](/docs/compiling.md)

### Running 

First download or build your binary (with the instructions above)
```sh
OPTIONS ./shinkai-node
```
OPTIONS can be found in [environment-variables.md](/docs/environment-variables.md)

## Contributions and Releases

Check our [contributions.md](/docs/contributions.md)


### Tips

if you want to restart the node, you can delete the folder `storage` and run the build again. More information at [https://docs.shinkai.com/getting-started](https://docs.shinkai.com/getting-started).

