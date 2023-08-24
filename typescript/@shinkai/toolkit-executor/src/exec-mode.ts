// Exec Mode
const util = require('node:util');
const exec = util.promisify(require('node:child_process').exec);
const fs = require('fs/promises');


async function runScript(src: string, env = ''): Promise<string> {
  const path = `./tmp_${new Date().getTime()}_${String(Math.random()).replace(
    /0./,
    ''
  )}.js`;

  await fs.writeFile(path, src, 'utf8');

  let result;

  try {
    const { error, stdout, stderr } = await exec(`${env} node ${path}`);

    if (error || stderr) {
      result = JSON.stringify({ stdout, error, stderr });
    } else {
      result = stdout;
    }
  } finally {
    // Ensure the temporary file is deleted
    try {
      await fs.unlink(path);
    } catch (err) {
      console.error(`Failed to delete temporary file: ${path}. Error:`, err);
    }
  }

  return result;
}

// Exec mode run once
export async function execMode(
  source: string,
  tool: string,
  input: string,
  headers: string
): Promise<string> {
  const src = `
  const tools = require('${source}'); 
  setTimeout(() => {
    (async () => {
      try {
        if (!tools['${tool}']) {
          console.log(JSON.stringify({ error: 'Tool "${tool}" not found' }));
          return;
        }
        const toolkit = new tools.ToolKitSetup;
        const tool = new tools['${tool}'];

        const rawHeaders = {};
        Object.assign(rawHeaders, ${headers || '{}'});
        const headers = await toolkit.processRawHeaderValues(rawHeaders);

        const rawInput = {};
        Object.assign(rawInput, ${input || '{}'});
        const inputData = await tool.validateInputs(rawInput);
        const inputObject = new tools.ShinkaiTookitLib.inputClass['${tool}']();
        Object.assign(inputObject, inputData);

        const response = await tool.run(inputObject, headers);

        console.log(JSON.stringify(response));
      } catch (e) {
        console.log(JSON.stringify({ error: e.message }));
      }
    })();
  }, 0);
  `;

  const parsedResponse = JSON.parse(await runScript(src));
  const toolResult = parsedResponse[tool as any];
  const response = { tool: tool, result: toolResult };
  return JSON.stringify(response);
}

export async function validate(
  source: string,
  headers: string
): Promise<string> {
  const src = `
  const tools = require('${source}'); 
  setTimeout(() => {
    (async () => {
      try {
        const toolkit = new tools.ToolKitSetup;
        const rawHeaders = {};
        Object.assign(rawHeaders, ${headers || '{}'});
        const response = await toolkit.validateHeaders(rawHeaders);
        console.log(JSON.stringify(response));
      } catch (e) {
        console.log(JSON.stringify({ error: e.message }));
      }
    })();
  }, 0);
  `;
  return await runScript(src);
}

export async function execModeConfig(source: string): Promise<string> {
  const src = `
    const tools = require('${source}');
  `;

  return await runScript(src, 'EMIT_TOOLS=1');
}
