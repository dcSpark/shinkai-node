process.stdout.on('error', err => {
  if (err.code === 'EPIPE') process.exit(0); // peer went away, exit silently
  else throw err;
});
