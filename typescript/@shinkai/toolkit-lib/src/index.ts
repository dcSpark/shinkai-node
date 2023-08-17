import {DecoratorsTools} from './DecoratorsTools';

export * from './decorators';
export * from './BaseTool';
export * from './types';
export * from './DecoratorsTools';
export * from './ShinkaiSetup';

// This async function processes the decorators and
// generates the tool descriptions and validators.
// Run at end of eventloop, after all user decorators are parsed.
DecoratorsTools.start();
(async () => {
  if (process.env.EMIT_TOOLS) {
    const config = await DecoratorsTools.emitConfig();
    console.log(config);
  }
})();
