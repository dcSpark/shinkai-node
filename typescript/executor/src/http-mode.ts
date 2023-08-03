import { execMode, execModeConfig } from "./exec-mode";
import fs from 'fs/promises';
// Http Mode
const express = require('express');
const bodyParser = require('body-parser');

export function httpMode(port: number) {
  const app = express();
  app.use(bodyParser.json({limit: '50mb'}));

  app.post('/config', async (req: any, res: any) => {
    if (!req.body.source) return res.status(400).json({ error: 'Missing source' });

    const path = `./tmp_${new Date().getTime()}_${String(Math.random()).replace(/0./, '')}.js`;
    await fs.writeFile(path, req.body.source, 'utf8');
    const response = await execModeConfig(path);
    await fs.unlink(path);
    res.json(JSON.parse(response));
  });

  app.post('/exec', async (req: any, res: any) => {
    if (!req.body.source) return res.status(400).json({ error: 'Missing source' });
    if (!req.body.tool) return res.status(400).json({ error: 'Missing tool' });

    const path = `./tmp_${new Date().getTime()}_${String(Math.random()).replace(/0./, '')}.js`;
    await fs.writeFile(path, req.body.source, 'utf8');
    const response = await execMode(path, req.body.tool, JSON.stringify(req.body.input) || '{}');
    await fs.unlink(path);
    res.json(JSON.parse(response));
  });

  app.listen(port, () => {
    console.log(`Listening at http://localhost:${port}`);
  });
}
