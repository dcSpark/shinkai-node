/* Internal Classes */
import {DecoratorsTools} from '@shinkai/toolkit-lib';
import {toolKitSetup} from './toolkitSetup';

// This async function processes the decorators and
// generates the tool descriptions and validators.
if (process.env.EMIT_TOOLS) {
  (async () => {
    await DecoratorsTools.start();
    const config = await DecoratorsTools.emitConfig(toolKitSetup);
    console.log(config);
  })();
}

/* Tools */
export {isEven} from './packages/is-even';
export {HTTP} from './packages/http';
export {GoogleCalendarQuickEvent} from './packages/activepieces-google-calendar/quick-event';
