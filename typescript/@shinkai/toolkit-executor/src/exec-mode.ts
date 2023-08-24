// Exec Mode
import {Worker} from 'node:worker_threads';
let processId = 0;

async function runScript(pid: number, src: string): Promise<Object> {
  const startTime = Date.now();
  return new Promise(resolve => {
    const worker = new Worker(src, {eval: true});
    worker.on('message', msg => {
      console.log(`< Process ${pid} finished in ${Date.now() - startTime}[ms]`);
      resolve(msg);
    });
  });
}

// Exec mode run once
export async function execMode(
  source: string,
  tool: string,
  input: string,
  headers: string
): Promise<{tool: string; result: unknown}> {
  const src = `
  ${source}
  ;

  const {ShinkaiToolkitLib, ToolKitSetup, ${tool}} = module.exports; 
  const {parentPort} = require('node:worker_threads');

  (async () => {
    try {
      if (!${tool}) {
        console.log(JSON.stringify({ error: 'Tool "${tool}" not found' }));
        return;
      }
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

      parentPort?.postMessage(response);
    } catch (e) {
      parentPort?.postMessage({ error: e.message });
    }
  })();
  `;
  processId += 1;
  console.log(`> EXEC Process ${processId}. Tool: ${tool}`);
  return {tool, result: await runScript(processId, src)};
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
      try {
        const toolkit = new ToolKitSetup();
        const rawHeaders = {};
        Object.assign(rawHeaders, ${headers || '{}'});
        const response = await toolkit.validateHeaders(rawHeaders);
        parentPort?.postMessage(response);
      } catch (e) {
        parentPort?.postMessage({ error: e.message });
      }
    })();
  `;
  processId += 1;
  console.log(`> VALIDATE Process ${processId}`);
  return {result: await runScript(processId, src)};
}

export async function toolkitConfig(source: string): Promise<Object> {
  const src = `
    ${source}
    ;

    const {ShinkaiToolkitLib, ToolKitSetup} = module.exports; 
    const {parentPort} = require('node:worker_threads');

    (async () => {
      try {
        const config = await ShinkaiToolkitLib.emitConfig();
        parentPort?.postMessage(config);
      } catch (e) {
        parentPort?.postMessage({ error: e.message });
      }
    })();
  `;
  processId += 1;
  console.log(`> TOOLKIT_CONFIG process ${processId}.`);
  return await runScript(processId, src);
}
