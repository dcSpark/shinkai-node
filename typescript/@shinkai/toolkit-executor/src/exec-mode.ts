import {log} from './log';
import {runScript} from './worker';

// Exec Mode
let processId = 0;

// Exec mode run once
export async function execMode(
  source: string,
  tool: string,
  input: string,
  headers: string
): Promise<{tool: string; outputs: Record<string, unknown>[]}> {
  const src = `
  ${source}
  ;

  const {ShinkaiToolkitLib, ToolKitSetup, ${tool}} = module.exports; 
  const {parentPort} = require('node:worker_threads');

  (async () => {
    const toolkit = new ToolKitSetup();
    const tool = new ${tool}();

    const rawHeaders = {};
    Object.assign(rawHeaders, ${headers || '{}'});
    const headers = await toolkit.processRawHeaderValues(rawHeaders);

    const rawInput = {};
    Object.assign(rawInput, ${input || '{}'});
    const inputData = await tool.validateInputs(rawInput);
    const inputObject = new ShinkaiToolkitLib.inputClass['${tool}']();
    Object.assign(inputObject, inputData);

    const response = await tool.run(inputObject, headers);
    parentPort?.postMessage(await response.processOutput());
  })();
  `;
  processId += 1;
  log(
    `[${new Date().toISOString()}] ⚒️ EXEC Process ${processId}. Tool: ${tool}`
  );
  return {tool, outputs: await runScript(processId, src)};
}

export async function validate(
  source: string,
  headers: string
): Promise<Object> {
  const src = `
    ${source}
    ;

    const {ShinkaiToolkitLib, ToolKitSetup} = module.exports; 
    const {parentPort} = require('node:worker_threads');

    (async () => {
      const toolkit = new ToolKitSetup();
      const rawHeaders = {};
      Object.assign(rawHeaders, ${headers || '{}'});
      const response = await toolkit.validateHeaders(rawHeaders);
      parentPort?.postMessage(response);
    })();
  `;
  processId += 1;
  log(`[${new Date().toISOString()}] 🧙 VALIDATE Process ${processId}`);
  return {result: await runScript(processId, src)};
}

export async function toolkitConfig(source: string): Promise<Object> {
  const src = `
    ${source}
    ;

    const {ShinkaiToolkitLib, ToolKitSetup} = module.exports; 
    const {parentPort} = require('node:worker_threads');

    (async () => {
      const config = await ShinkaiToolkitLib.emitConfig();
      parentPort?.postMessage(config);
    })();
  `;
  processId += 1;
  console.log(
    `[${new Date().toISOString()}] ⚙️ TOOLKIT_CONFIG process ${processId}.`
  );
  return await runScript(processId, src);
}
