## Local build

Inside the folder `docker-build` run:

```sh
DOCKER_BUILDKIT=1 docker build -f Dockerfile-RELEASE -t dcspark/shinkai-node:latest ../
```

Inside the folder `docker-build`, to start the services, run:

```sh
INITIAL_AGENT_API_KEYS=sk-abc,sk-abc docker compose up -d
```

The following configuration items can be set from environment:
- __INITIAL_AGENT_NAMES__=${INITIAL_AGENT_NAMES:-openai_gpt,openai_gpt_vision}
- __INITIAL_AGENT_MODELS__=${INITIAL_AGENT_MODELS:-openai:gpt-4-1106-preview,openai:gpt-4-vision-preview}
- __INITIAL_AGENT_URLS__=${INITIAL_AGENT_URLS:-https://api.openai.com,https://api.openai.com}
- __INITIAL_AGENT_API_KEYS__=${INITIAL_AGENT_API_KEYS}

Point Visor to `http://127.0.0.1:9550`

## Prepare for partner

Inside the folder `docker-build` run:

```sh
sh partner_prepare.sh
```

output example:

```sh
$ sh partner_prepare.sh

[INFO]  Docker building dcspark/shinkai-node:latest using Dockerfile-RELEASE with source at ../
sha256:b5fe5c4c8fc6229c15ea0cbde4881c090a0dcd72a1f6f8f42d29d7f9bfc8b4be
[INFO]  Preparing docker compose environment at shinkai_deploy
[INFO]  Docker save dcspark/shinkai-node:latest to dcspark_shinkai-node.tar
[INFO]  Preparing partner data at shinkai_deploy_partner/shinkai_deploy.tar.gz
[INFO]  Cleaning shinkai_deploy
[INFO]  Send to partner the file shinkai_deploy_partner/shinkai_deploy.tar.gz
```

Send to partner the final output generated at `shinkai_deploy_partner/shinkai_deploy.tar.gz`

## Partner info

Partner extracts the file `tar xzvf shinkai_deploy.tar.gz`

```sh
$ tar xzvf shinkai_deploy.tar.gz

shinkai_deploy/
shinkai_deploy/.env
shinkai_deploy/docker-compose.yml
shinkai_deploy/prepare.sh
shinkai_deploy/dcspark_shinkai-node.tar
```

and ends up with a folder `shinkai_deploy` containing:

```sh
shinkai_deploy
├── dcspark_shinkai-node.tar
├── docker-compose.yml
├── .env
└── prepare.sh
```

runs `sh prepare.sh` that outputs additional information:

```sh
$ sh prepare.sh

[INFO]  Docker loading dcspark_shinkai-node.tar
Loaded image: dcspark/shinkai-node:latest
[INFO]  Edit ".env" if you want to start the node with preconfigured ai agents. You have the possibility to add ai agents also from Shinkai Visor.
[INFO]  Once done with ".env" changes, to start on-prem infrastructure run: docker compose up -d
[INFO]  Once everything is up and running, install/start Shinkai Visor and use the default provided settings on the ui.
```

final step is to run `docker compose up -d`.