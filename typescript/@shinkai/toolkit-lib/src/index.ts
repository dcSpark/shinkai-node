import {ShinkaiTookitLib} from './ShinkaiTookitLib';

export * from './decorators';
export * from './BaseTool';
export * from './types';
export * from './ShinkaiTookitLib';
export * from './ShinkaiSetup';

// This async function processes the decorators and
// generates the tool descriptions and validators.
// Run at end of eventloop, after all user decorators are parsed.
ShinkaiTookitLib.start();
(async () => {
  if (process.env.EMIT_TOOLS) {
    const config = await ShinkaiTookitLib.emitConfig();
    console.log(config);
  }
})();
