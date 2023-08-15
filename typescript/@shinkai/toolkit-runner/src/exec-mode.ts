// Exec Mode
const util = require('node:util');
const exec = util.promisify(require('node:child_process').exec);
const fs = require('fs/promises');

async function runScript(src: string, env: string = '') {
  // Create a temporal file for execution.
  const path = `./tmp_${new Date().getTime()}_${String(Math.random()).replace(/0./, '')}.js`;
  await fs.writeFile(path, src, 'utf8');
  const { error, stdout, stderr } = await exec(`${env} node ${path}`);
  await fs.unlink(path);

  if (error || stderr) {
    return { stdout, error, stderr};
  }

  return stdout;
}

// Exec mode run once
export async function execMode(source: string, tool: string, input: string, headers: string): Promise<any> {
  const src = `
  const tools = require('${source}'); 
  setTimeout(() => {
    (async () => {
      try {
        if (!tools['${tool}']) {
          console.log(JSON.stringify({ error: 'Tool "${tool}" not found' }));
          return;
        }
        const tool = new tools['${tool}'];
        const input = new tools.DecoratorsTools.classMap['${tool}']();
        const headers = {};
        Object.assign(input, ${input || '{}'});
        Object.assign(headers, ${headers || '{}'});
        const response = await tool.run(input, headers);
        console.log(JSON.stringify(response));
      } catch (e) {
        console.log(JSON.stringify({ error: e.message }));
      }
    })();
  }, 0);
  `;
  return await runScript(src);
}

export async function execModeConfig(source: string): Promise<any> {
  const src = `
    const tools = require('${source}');
  `;

  return await runScript(src, 'EMIT_TOOLS=1');
}
