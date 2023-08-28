import {Worker} from 'node:worker_threads';
import {log} from './log';

const TIMEOUT = process.env.WORKER_TIMEOUT_MS
  ? parseInt(process.env.WORKER_TIMEOUT_MS, 10)
  : 120 * 1000; // 120 seconds;

export async function runScript<T>(pid: number, src: string): Promise<T> {
  const startTime = Date.now();
  let resolved = false;

  return new Promise(resolve => {
    const worker = new Worker(src, {eval: true});

    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const end = (message: any) => {
      if (!resolved) {
        clearTimeout(timeoutId);
        resolved = true;
        resolve(message);
      }
    };

    const timeoutId = setTimeout(() => {
      log(`[${new Date().toISOString()}] ❌ Process ${pid} timed out`);
      worker?.terminate();
    }, TIMEOUT);

    worker.on('message', msg => {
      log(
        `[${new Date().toISOString()}] 🏁 Process ${pid} finished in ${
          Date.now() - startTime
        }[ms]`
      );
      end(msg);
    });

    worker.on('error', err => {
      log(
        `[${new Date().toISOString()}] ❌ Process ${pid} errored in ${
          Date.now() - startTime
        }[ms] error: ${err.message}`
      );
      end({error: err.message});
    });

    worker.on('exit', code => {
      if (!resolved) {
        log(
          `[${new Date().toISOString()}] ❌ Process ${pid} exited in ${
            Date.now() - startTime
          }[ms] code: ${code}`
        );
      }
      end({errorCode: code});
    });
  });
}
