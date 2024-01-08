#!/bin/sh
set -e
set -o noglob


SED_COMMAND=sed
COPY_COMMAND=cp

SHINKAI_NODE_IMAGE=${SHINKAI_NODE_IMAGE:-dcspark/shinkai-node}
SHINKAI_NODE_VERSION=${SHINKAI_NODE_VERSION:-latest}

SHINKAI_COMPOSE_FILE=docker-compose.yml
SHINKAI_NODE_DOCKERFILE=Dockerfile-RELEASE

SHINKAI_NODE_ARCHIVE=dcspark_shinkai-node.tar
SHINKAI_SOURCE_PATH=../

DOCKER_BUILD_CMD="docker build --quiet"
DOCKER_COMPOSE_CMD="docker compose" # docker-compose
DOCKER_LOAD_CMD="docker load --input"
DOCKER_SAVE_CMD="docker save --output"

SHINKAI_TMP_LOCAL_FOLDER=shinkai_deploy
SHINKAI_TMP_PARTNER_FOLDER=shinkai_deploy_partner
DOCKER_COMPOSE_ENV_FILE=.env
DOCKER_COMPOSE_ENV_DATA=$(cat << EOF
#
# single agent example
#
#INITIAL_AGENT_NAMES=openai_gpt
#INITIAL_AGENT_URLS=https://api.openai.com
#INITIAL_AGENT_MODELS=openai:gpt-4-1106-preview
#INITIAL_AGENT_API_KEYS=sk-abc
#
# multi agent example
#
#INITIAL_AGENT_NAMES=openai_gpt,openai_gpt_vision
#INITIAL_AGENT_URLS=https://api.openai.com,https://api.openai.com
#INITIAL_AGENT_MODELS=openai:gpt-4-1106-preview,openai:gpt-4-vision-preview
#INITIAL_AGENT_API_KEYS=sk-abc,sk-abc
#
# default none
#
INITIAL_AGENT_NAMES=
INITIAL_AGENT_URLS=
INITIAL_AGENT_MODELS=
INITIAL_AGENT_API_KEYS=
EOF
)



PARTNER_PREPARE_SCRIPT=$(cat << EOF
#!/bin/sh
set -e
set -o noglob

SHINKAI_NODE_ARCHIVE=dcspark_shinkai-node.tar
DOCKER_LOAD_CMD="docker load --input"
DOCKER_COMPOSE_CMD="docker compose" # docker-compose
DOCKER_COMPOSE_ENV_FILE=.env


# --- helper functions for logs ---
info() {
  echo '[INFO] ' "\$@"
}
warn() {
  echo '[WARN] ' "\$@" >&2
}
fatal() {
  echo '[ERRO] ' "\$@" >&2
  exit 1
}

# --- load image ---
load_docker_image() {
  msg="Docker loading \${SHINKAI_NODE_ARCHIVE}"
  if [ -f \${SHINKAI_NODE_ARCHIVE} ]; then
    info \${msg}
    \${DOCKER_LOAD_CMD} \${SHINKAI_NODE_ARCHIVE}
  else
    fatal "\${msg} - failed (missing file - \${SHINKAI_NODE_ARCHIVE})"
  fi
}

# --- info about initial agents configuration ---
post_prepare_env_info() {
  msg="Edit \"\${DOCKER_COMPOSE_ENV_FILE}\" if you want to start the node with preconfigured ai agents. You have the possibility to add ai agents also from Shinkai Visor."
  info \${msg}
}

# --- info docker compose ---
post_prepare_compose_info() {
  msg="Once done with \"\${DOCKER_COMPOSE_ENV_FILE}\" changes, to start on-prem infrastructure run: \${DOCKER_COMPOSE_CMD} up -d"
  info \${msg}
}


# --- info visor ---
post_prepare_visor_info() {
  msg="Once everything is up and running, install/start Shinkai Visor and use the default provided settings on the ui."
  info \${msg}
}

load_docker_image
post_prepare_env_info
post_prepare_compose_info
post_prepare_visor_info

EOF
)

# --- helper functions for logs ---
info() {
  echo '[INFO] ' "$@"
}
warn() {
  echo '[WARN] ' "$@" >&2
}
fatal() {
  echo '[ERRO] ' "$@" >&2
  exit 1
}

# write $1 (content) to $2 (file)
write_to_file() {
  echo "$1" >| "$2" || fatal "failed to write data to $2"
}

# --- build image ---
build_docker_image() {
  msg="Docker building ${SHINKAI_NODE_IMAGE}:${SHINKAI_NODE_VERSION} using ${SHINKAI_NODE_DOCKERFILE} with source at ${SHINKAI_SOURCE_PATH}"
  if [ -f ${SHINKAI_NODE_DOCKERFILE} ]; then
    info ${msg}
    export DOCKER_BUILDKIT=1
    ${DOCKER_BUILD_CMD} -f ${SHINKAI_NODE_DOCKERFILE} -t ${SHINKAI_NODE_IMAGE}:${SHINKAI_NODE_VERSION} ${SHINKAI_SOURCE_PATH}
  else
    fatal "${msg} - failed (missing file - ${SHINKAI_NODE_DOCKERFILE})"
  fi
}

# --- save image ---
save_docker_image() {
  if [ ! -d "${SHINKAI_TMP_LOCAL_FOLDER}" ]; then
    mkdir ${SHINKAI_TMP_LOCAL_FOLDER} || fatal "failed to create local folder ${SHINKAI_TMP_LOCAL_FOLDER}"
  fi
  msg="Docker save ${SHINKAI_NODE_IMAGE}:${SHINKAI_NODE_VERSION} to ${SHINKAI_NODE_ARCHIVE}"
  if [ ! -f ${SHINKAI_NODE_ARCHIVE} ]; then
    info ${msg}
    ${DOCKER_SAVE_CMD} ${SHINKAI_TMP_LOCAL_FOLDER}/${SHINKAI_NODE_ARCHIVE} ${SHINKAI_NODE_IMAGE}:${SHINKAI_NODE_VERSION}
  else
    fatal "${msg} - failed (file already exists - ${SHINKAI_TMP_LOCAL_FOLDER}/${SHINKAI_NODE_ARCHIVE})"
  fi
}

# --- prepare docker-compose infra for partner ---
prepare_docker_compose() {
  msg="Preparing docker compose environment at ${SHINKAI_TMP_LOCAL_FOLDER}"
  if [ ! -d "${SHINKAI_TMP_LOCAL_FOLDER}" ]; then
    mkdir ${SHINKAI_TMP_LOCAL_FOLDER} || fatal "failed to create local folder ${SHINKAI_TMP_LOCAL_FOLDER}"
  fi
  info ${msg}
  # copy original compose file
  ${COPY_COMMAND} ${SHINKAI_COMPOSE_FILE} ${SHINKAI_TMP_LOCAL_FOLDER}/${SHINKAI_COMPOSE_FILE} || fatal "failed to copy ${SHINKAI_TMP_LOCAL_FOLDER}/${SHINKAI_COMPOSE_FILE}"
  # replace any INITIAL_AGENT_* initial value with envs
  ${SED_COMMAND} -i "s/INITIAL_AGENT_NAMES=.*/INITIAL_AGENT_NAMES=\${INITIAL_AGENT_NAMES}/g" ${SHINKAI_TMP_LOCAL_FOLDER}/${SHINKAI_COMPOSE_FILE} || fatal 'sed failed - INITIAL_AGENT_NAMES'
  ${SED_COMMAND} -i "s/INITIAL_AGENT_URLS=.*/INITIAL_AGENT_URLS=\${INITIAL_AGENT_URLS}/g" ${SHINKAI_TMP_LOCAL_FOLDER}/${SHINKAI_COMPOSE_FILE} || fatal 'sed failed - INITIAL_AGENT_URLS'
  ${SED_COMMAND} -i "s/INITIAL_AGENT_MODELS=.*/INITIAL_AGENT_MODELS=\${INITIAL_AGENT_MODELS}/g" ${SHINKAI_TMP_LOCAL_FOLDER}/${SHINKAI_COMPOSE_FILE} || fatal 'sed failed - INITIAL_AGENT_MODELS'
  ${SED_COMMAND} -i "s/INITIAL_AGENT_API_KEYS=.*/INITIAL_AGENT_API_KEYS=\${INITIAL_AGENT_API_KEYS}/g" ${SHINKAI_TMP_LOCAL_FOLDER}/${SHINKAI_COMPOSE_FILE} || fatal 'sed failed - INITIAL_AGENT_API_KEYS'
  # write .env sample file
  write_to_file "${DOCKER_COMPOSE_ENV_DATA}" ${SHINKAI_TMP_LOCAL_FOLDER}/${DOCKER_COMPOSE_ENV_FILE}
  # write partner prepare.sh
  write_to_file "${PARTNER_PREPARE_SCRIPT}" ${SHINKAI_TMP_LOCAL_FOLDER}/prepare.sh

}

# --- load image ---
load_docker_image() {
  msg="Docker loading ${SHINKAI_NODE_ARCHIVE}"
  if [ -f ${SHINKAI_NODE_ARCHIVE} ]; then
    info ${msg}
    ${DOCKER_LOAD_CMD} ${SHINKAI_NODE_ARCHIVE}
  else
    fatal "${msg} - failed (missing file - ${SHINKAI_NODE_ARCHIVE})"
  fi
}

# --- prepare partner archive ---
prepare_partner_archive() {
  msg="Preparing partner data at ${SHINKAI_TMP_PARTNER_FOLDER}/${SHINKAI_TMP_LOCAL_FOLDER}.tar.gz"
  if [ ! -d "${SHINKAI_TMP_PARTNER_FOLDER}" ]; then
    mkdir ${SHINKAI_TMP_PARTNER_FOLDER} || fatal "failed to create local folder ${SHINKAI_TMP_PARTNER_FOLDER}"
  fi
  info ${msg}
  tar -zcf ${SHINKAI_TMP_PARTNER_FOLDER}/${SHINKAI_TMP_LOCAL_FOLDER}.tar.gz ${SHINKAI_TMP_LOCAL_FOLDER}
}


# --- clean temp partner folder ---
clean_temporary_folder() {
  msg="Cleaning ${SHINKAI_TMP_LOCAL_FOLDER}"
  if [ -d "${SHINKAI_TMP_LOCAL_FOLDER}" ]; then
    info ${msg}
    rm -rf ${SHINKAI_TMP_LOCAL_FOLDER} || fatal "failed delete local folder ${SHINKAI_TMP_LOCAL_FOLDER}"
  fi
}

# --- info what to send to partner  ---
partner_file_info() {
  msg="Send to partner the file ${SHINKAI_TMP_PARTNER_FOLDER}/${SHINKAI_TMP_LOCAL_FOLDER}.tar.gz"
  if [ -f "${SHINKAI_TMP_PARTNER_FOLDER}/${SHINKAI_TMP_LOCAL_FOLDER}.tar.gz" ]; then
    info ${msg}
  else
    fatal "${msg} - error (missing file - ${SHINKAI_TMP_PARTNER_FOLDER}/${SHINKAI_TMP_LOCAL_FOLDER}.tar.gz)"
  fi
}


build_docker_image
prepare_docker_compose
save_docker_image
prepare_partner_archive
clean_temporary_folder
partner_file_info