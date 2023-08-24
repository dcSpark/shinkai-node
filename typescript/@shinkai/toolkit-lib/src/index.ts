import {ShinkaiToolkitLib} from './ShinkaiToolkitLib';

export * from './decorators';
export * from './BaseTool';
export * from './types';
export * from './ShinkaiToolkitLib';
export * from './ShinkaiSetup';

// This async function processes the decorators and
// generates the tool descriptions and validators.
// Run at end of eventloop, after all user decorators are parsed.
ShinkaiToolkitLib.start();
(async () => {
  if (process.env.EMIT_TOOLS) {
    const config = await ShinkaiToolkitLib.emitConfig();
    console.log(config);
  }
})();
