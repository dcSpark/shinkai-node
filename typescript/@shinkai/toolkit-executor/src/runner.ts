/* eslint-disable no-process-exit */
import {program} from 'commander';
import {execMode, toolkitConfig, validate} from './exec-mode';
import {httpMode} from './http-mode';
// eslint-disable-next-line no-restricted-imports
import fs from 'fs/promises';

program
  .option('-e, --exec-mode', 'Execution mode: exec')
  .option('-w, --http-mode', 'Execution mode: http')
  .option('-s, --source <string>', 'For exec-mode, path to the source file')
  .option(
    '-c, --get-config',
    'For exec-mode, extract the config from the source file'
  )
  .option('-v, --validate', 'For exec-mode, validate the headers')
  .option('-t, --tool <string>', 'For exec-mode, name of the tool to execute')
  .option(
    '-i, --input <json-string>',
    'For exec-mode, input data as a JSON string'
  )
  .option(
    '-x, --headers <json-string>',
    'For exec-mode, headers as a JSON string'
  )
  .option('-p, --port <number>', 'For http-mode, port to listen to', '3000');

program.parse();
const options = program.opts();

function validateOptions() {
  if (options.httpMode && options.execMode) {
    console.log('Cannot use both execution modes at the same time');
    process.exit(1);
  }

  if (!options.httpMode && !options.execMode) {
    console.log('Must use one execution mode: -e (exec) or -w (http)');
    process.exit(1);
  }

  if (options.execMode) {
    if (!options.source) {
      console.log('Must provide a source file path: -s <path>');
      process.exit(1);
    }

    if (options.getConfig) {
      // no other options needed for extrating config
    } else if (options.validate) {
      // only headers required
      if (!options.headers) {
        console.log('Must provide headers: -x <headers>');
        process.exit(1);
      }
    } else {
      // standard execution mode
      if (!options.tool) {
        console.log('Must provide a tool name: -t <name>');
        process.exit(1);
      }
      if (!options.input) {
        console.log('Must provide input: -i <input>');
        process.exit(1);
      }
    }
  }
}

validateOptions();

(async () => {
  if (options.execMode) {
    const source = await fs.readFile(options.source, 'utf8');

    if (options.validate) {
      console.log(
        JSON.stringify(await validate(source, options.headers), null, 2)
      );
    } else if (options.getConfig) {
      console.log(JSON.stringify(await toolkitConfig(source), null, 2));
    } else {
      console.log(
        JSON.stringify(
          await execMode(source, options.tool, options.input, options.headers),
          null,
          2
        )
      );
    }
  } else if (options.httpMode) {
    httpMode(options.port);
  }
})();
