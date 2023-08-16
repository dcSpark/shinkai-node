import {DecoratorsTools} from './DecortorsTools';

export * from './decorators';
export * from './BaseTool';
export * from './types';

// This async function processes the decorators and
// generates the tool descriptions and validators.
// Run at end of eventloop, after all user decorators are parsed.
setTimeout(() => {
  DecoratorsTools.start();
}, 0);

(async () => {
  if (process.env.EMIT_TOOLS) {
    const config = await DecoratorsTools.emitConfig();
    console.log(config);
  }
})();
