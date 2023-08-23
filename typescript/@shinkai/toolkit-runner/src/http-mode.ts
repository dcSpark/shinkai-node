import {execMode, execModeConfig, validate} from './exec-mode';
import fs from 'fs/promises';
// Http Mode
import express from 'express';
import bodyParser from 'body-parser';
import {IncomingHttpHeaders} from 'http';

export function httpMode(port: string | number) {
  const app = express();
  app.use(bodyParser.json({limit: '50mb'}));

  app.post(
    '/validate',
    async (
      req: express.Request<{}, {}, {source: string}>,
      res: express.Response
    ) => {
      if (!req.body.source)
        return res.status(400).json({error: 'Missing source'});

      const response = await runWithSource(
        req.body.source,
        async path => await validate(path, filterHeaders(req.headers))
      );

      return res.json(JSON.parse(response));
    }
  );

  app.post(
    '/toolkit_json',
    async (
      req: express.Request<{}, {}, {source: string}>,
      res: express.Response
    ) => {
      if (!req.body.source)
        return res.status(400).json({error: 'Missing source'});

      const response = await runWithSource(
        req.body.source,
        async path => await execModeConfig(path)
      );

      return res.json(JSON.parse(response));
    }
  );

  app.post(
    '/exec',
    async (
      req: express.Request<
        {},
        {},
        {source: string; tool: string; input: string}
      >,
      res: express.Response
    ) => {
      if (!req.body) return res.status(400).json({error: 'Missing body'});
      if (!req.body.source)
        return res.status(400).json({error: 'Missing source'});
      if (!req.body.tool) return res.status(400).json({error: 'Missing tool'});

      const response = await runWithSource(
        req.body.source,
        async path =>
          await execMode(
            path,
            req.body.tool,
            JSON.stringify(req.body.input || {}),
            filterHeaders(req.headers)
          )
      );

      return res.send(response); //  res.json(JSON.parse(response));
    }
  );

  app.all(
    '/healthcheck',
    async (req: express.Request, res: express.Response) => {
      return res.json({status: true});
    }
  );

  app.listen(parseInt(String(port), 10), () => {
    console.log(`Listening at http://localhost:${port}`);
  });
}

const filterHeaders = (rawHeaders: IncomingHttpHeaders): string => {
  const headers: Record<string, string | string[] | undefined> = {};
  Object.keys(rawHeaders || {}).forEach(h => {
    if (h.match(/^x-shinkai-.*/)) {
      headers[h] = rawHeaders[h];
    }
  });
  return JSON.stringify(headers);
};

const runWithSource = async <T>(
  source: string,
  callback: (path: string) => Promise<T>
): Promise<T> => {
  const path = `./tmp_${new Date().getTime()}_${String(Math.random()).replace(
    /0./,
    ''
  )}.js`;
  await fs.writeFile(path, source, 'utf8');
  const data = await callback(path);
  await fs.unlink(path);
  return data;
};
