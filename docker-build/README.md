On main repo of shinkai-node run:
```
DOCKER_BUILDKIT=1 docker build --rm --compress -f docker-build/Dockerfile-RELEASE -t dcspark/shinkai-node:latest .
```

Then inside the folder `docker-build` run:

```sh
INITIAL_AGENT_API_KEYS=sk-abc,sk-abc docker compose up -d
```
The following configuration items can be set from environment:
- __INITIAL_AGENT_NAMES__=${INITIAL_AGENT_NAMES:-openai_gpt,openai_gpt_vision}
- __INITIAL_AGENT_MODELS__=${INITIAL_AGENT_MODELS:-openai:gpt-4-1106-preview,openai:gpt-4-vision-preview}
- __INITIAL_AGENT_URLS__=${INITIAL_AGENT_URLS:-https://api.openai.com,https://api.openai.com}
- __INITIAL_AGENT_API_KEYS__=${INITIAL_AGENT_API_KEYS}


Point Visor to `http://127.0.0.1:9550`
