import { execMode, execModeConfig } from "./exec-mode";
import fs from 'fs/promises';
// Http Mode
import express from 'express';
import bodyParser from 'body-parser';

export function httpMode(port: number) {
  const app = express();
  app.use(bodyParser.json({limit: '50mb'}));

  app.post('/config', async (req: express.Request<{}, {}, {source: string}>, res: express.Response) => {
    if (!req.body.source) return res.status(400).json({ error: 'Missing source' });

    const path = `./tmp_${new Date().getTime()}_${String(Math.random()).replace(/0./, '')}.js`;
    await fs.writeFile(path, req.body.source, 'utf8');
    const response = await execModeConfig(path);
    await fs.unlink(path);
    res.json(JSON.parse(response));
  });

  app.post('/exec', async (req: express.Request<{}, {}, {source: string, tool: string, input: string}>, res: express.Response) => {
    if (!req.body) return res.status(400).json({ error: 'Missing body' });
    if (!req.body.source) return res.status(400).json({ error: 'Missing source' });
    if (!req.body.tool) return res.status(400).json({ error: 'Missing tool' });
    const input = JSON.stringify(req.body.input || {});
    const headers: Record<string, string | string[] | undefined> = {};
    Object.keys(req.headers || {}).forEach(h => {
      if (h.match(/^x-shinkai-.*/)) {
        headers[h] = req.headers[h];
      }
    });
    
    const path = `./tmp_${new Date().getTime()}_${String(Math.random()).replace(/0./, '')}.js`;
    await fs.writeFile(path, req.body.source, 'utf8');

    const response = await execMode(path, req.body.tool, input, JSON.stringify(headers));
    await fs.unlink(path);
    res.json(JSON.parse(response));
  });

  app.listen(port, () => {
    console.log(`Listening at http://localhost:${port}`);
  });
}
