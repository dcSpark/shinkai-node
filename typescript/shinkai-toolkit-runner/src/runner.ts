import { program } from 'commander';
import { execMode, execModeConfig } from './exec-mode';
import { httpMode } from './http-mode';

program
  .option('-e, --exec-mode', 'Execution mode: exec')
  .option('-w, --http-mode', 'Execution mode: http')
  .option('-c, --get-config', 'For exec-mode, extract the config from the source file')
  .option('-s, --source <string>', 'For exec-mode, path to the source file')
  .option('-t, --tool <string>', 'For exec-mode, name of the tool to execute')
  .option('-i, --input <json-string>', 'For exec-mode, input data as a JSON string')
  .option('-p, --port <number>', 'For http-modem, port to listen to', 3000 as any);

program.parse();
const options = program.opts();

function validate() {
  if (options.httpMode && options.execMode) {
    console.log('Cannot use both execution modes at the same time');
    process.exit(1);
  }
  
  if (!options.httpMode && !options.execMode) {
    console.log('Must use one execution mode: -e (exec) or -h (http)');
    process.exit(1);
  }

  if (options.execMode) {
    if (!options.getConfig) {
      if (!options.source) {
        console.log('Must provide a source file path: -s <path>');
        process.exit(1);
      }
      if (!options.tool) {
        console.log('Must provide a tool name: -t <name>');
        process.exit(1);
      }
    }
  }
}

validate();

(async () => {
  if (options.execMode) {
    if (options.getConfig) {
      console.log(await execModeConfig(options.source));
    } else {
      console.log(await execMode(options.source, options.tool, options.input));
    }
  } else if (options.httpMode) {
    httpMode(options.port);
  }
})();