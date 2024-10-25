# Default Ollama url:port
EMBEDDINGS_SERVER_URL=http://localhost:11434;

case "$OSTYPE" in

  linux*)   
    echo "Installing for Linux" ;
    # Default Ollama port

     # Check if unzip is installed
    if ! command -v unzip &> /dev/null; then
        echo "Error: unzip is not installed. Please install unzip to continue."
        echo "Install unzip with `brew install unzip`"
        exit 1
    fi

    # Check if Ollama is running
    if curl -s "$EMBEDDINGS_SERVER_URL" | grep -q "Ollama is running"; then
        echo "Embeddings server found is running and ready."
    else
        echo "Error: Ollama is not running or not responding as expected."
        echo "Please make sure Ollama is installed and running on $EMBEDDINGS_SERVER_URL"
        echo ""
        echo "You can download ollama from https://ollama.com/download"
        exit 1
    fi

    curl -L -o shinkai-node.zip https://github.com/dcSpark/shinkai-node/releases/latest/download/shinkai-node-x86_64-unknown-linux-gnu.zip
    unzip shinkai-node.zip -d shinkai-node;

    FULL_PATH=`pwd`/shinkai-node/shinkai-node
    
    echo "Instalation complete!"
    echo "---------------------"
    echo "Now to run shinkai-node run:"
    echo ""
    echo "EMBEDDINGS_SERVER_URL=$EMBEDDINGS_SERVER_URL $FULL_PATH"
    echo ""

    ;;
  darwin*)
    echo "Installing for MacOS" ;

    # Check if unzip is installed
    if ! command -v unzip &> /dev/null; then
        echo "Error: unzip is not installed. Please install unzip to continue."
        echo "Install unzip with `brew install unzip`"
        exit 1
    fi

    # Check if Ollama is running
    if curl -s "$EMBEDDINGS_SERVER_URL" | grep -q "Ollama is running"; then
        echo "Embeddings server found is running and ready."
    else
        echo "Error: Ollama is not running or not responding as expected."
        echo "Please make sure Ollama is installed and running on $EMBEDDINGS_SERVER_URL"
        echo ""
        echo "You can download ollama from https://ollama.com/download"
        exit 1
    fi

    curl -L -o shinkai-node.zip https://github.com/dcSpark/shinkai-node/releases/latest/download/shinkai-node-aarch64-apple-darwin.zip
    unzip shinkai-node.zip -d shinkai-node;

    FULL_PATH=`pwd`/shinkai-node/shinkai-node
    
    echo "Instalation complete!"
    echo "---------------------"
    echo "Now to run shinkai-node run:"
    echo ""
    echo "EMBEDDINGS_SERVER_URL=$EMBEDDINGS_SERVER_URL $FULL_PATH"
    echo ""
    ;;
  msys*)    
    echo "NYI for Windows"
    ;;
  *) 
    echo "Unknown OS: $OSTYPE"
    ;;
esac