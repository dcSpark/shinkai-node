const LOG = process.env.LOG === 'false' ? false : true;

export function log(...args: unknown[]) {
  if (LOG) {
    console.log(...args);
  }
}
