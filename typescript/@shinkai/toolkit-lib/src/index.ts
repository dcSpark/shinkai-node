import {DecoratorsTools} from './Decortors';

export * from './Decortors';
export * from './BaseTool';

// This async function processes the decorators and
// generates the tool descriptions and validators.
if (process.env.EMIT_TOOLS) {
  (async () => {
    await DecoratorsTools.start();
    const config = await DecoratorsTools.emitConfig();
    console.log(config);
  })();
}
