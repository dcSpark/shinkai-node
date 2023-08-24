import {execMode, toolkitConfig, validate} from './exec-mode';

// Http Mode
import express from 'express';
import bodyParser from 'body-parser';
import {IncomingHttpHeaders} from 'http';

export function httpMode(port: string | number) {
  const app = express();
  app.use(bodyParser.json({limit: '50mb'}));

  app.post(
    '/validate_headers',
    async (
      req: express.Request<{}, {}, {source: string}>,
      res: express.Response
    ) => {
      if (!req.body.source)
        return res.status(400).json({error: 'Missing source'});

      return res.json(
        await validate(req.body.source, filterHeaders(req.headers))
      );
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
      return res.json(await toolkitConfig(req.body.source));
    }
  );

  app.post(
    '/execute_tool',
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

      return res.json(
        await execMode(
          req.body.source,
          req.body.tool,
          JSON.stringify(req.body.input || {}),
          filterHeaders(req.headers)
        )
      );
    }
  );

  app.all(
    '/health_check',
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
