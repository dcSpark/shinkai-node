// eslint-disable-next-line node/no-unsupported-features/node-builtins
import {isMainThread} from 'worker_threads';

if (isMainThread) {
  require('./runner');
}
