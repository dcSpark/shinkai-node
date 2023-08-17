#!/usr/bin/env node

/*
 * This script is used to create a new tool in the toolkit.
 *  
 * Usage:
 * ./new-tool.js <tool-name>
 *  
 * It copies the base toolkit content into a new folder,
 * and updates names.
 */

const fs = require('fs');
const path = require('path');

// Check Node.js version
if (!fs.cpSync) {
  console.log('This script requires Node.js 16+ or higher');
  process.exit(1);
}

// Check arguments
const args = process.argv.slice(2);
if (args.length !== 1 || !args[0].length) {
  console.log('Usage: new-tool.js <tool-name>');
  process.exit(1);
}

const toolName = args[0];

if (toolName.match(/[^a-zA-Z_]/)) {
  console.log('Tool name can only contain letters and underscores');
  process.exit(1);
}

const typescryptKeywords = ['async', 'await', 'break', 'case', 'catch', 
  'class', 'const', 'continue', 'debugger', 'default', 'delete', 'do', 'else',
  'enum', 'export', 'extends', 'false', 'finally', 'for', 'function', 'if', 
  'import', 'in', 'instanceof', 'new', 'namespace', 'null', 'return', 'super',
  'switch', 'this', 'throw', 'true', 'try', 'typeof', 'var', 'void', 'while',
  'with'];

if (typescryptKeywords.includes(toolName)) {
  console.log(`Tool name cannot be a resvered TypeScript keyword: ${toolName}`, typescryptKeywords);
  process.exit(1);
}

const shinkaiKeywords = ['test'];

if (shinkaiKeywords.includes(toolName)) {
  console.log(`Tool name cannot be a reserved Shinkai keyword: ${toolName}`, shinkaiKeywords);
  process.exit(1);
}

const targetFolder = `./${toolName}`;

// Create folder
if (!fs.existsSync(targetFolder)) {
  fs.mkdirSync(targetFolder);
} else {
  console.log(`Folder ${targetFolder} already exists`);
  process.exit(1);
}

// Copy files
const src = path.join(__dirname, 'lib');
fs.cpSync(src, targetFolder, {recursive: true});

// Replace sample dir with "toolName"
const sampleDirectory = path.join(toolName, 'src', 'packages', 'sample');
const toolDirectory = path.join(toolName, 'src', 'packages', toolName.toLowerCase());

// Replace folder
fs.cpSync(sampleDirectory, toolDirectory, {recursive: true});
fs.rmSync(sampleDirectory, { recursive: true, force: true });

const replaceContents = (filePath, searchValue, replaceValue) => {
  const fileContents = fs.readFileSync(filePath, 'utf8');
  const newFileContents = fileContents.replace(new RegExp(searchValue, 'g'), replaceValue);
  fs.writeFileSync(filePath, newFileContents, 'utf8');
};

// Replace Class
const toolIndex = path.join(toolName, 'src', 'packages', toolName.toLowerCase(), 'index.ts');
replaceContents(toolIndex, 'Sample', toolName);

// Replace Package.json
const packageJson = path.join(toolName, 'package.json');
replaceContents(packageJson, 'sample', toolName.toLocaleLowerCase());
// TODO REPLACE WITH @shinkai/toolkit-lib
replaceContents(packageJson, 'file:../../toolkit-lib', 'file:../toolkit-lib');

// Replace Exports
const toolExports = path.join(toolName, 'src', 'Registry.ts');
replaceContents(toolExports, 'Sample', toolName);
replaceContents(toolExports, 'sample', toolName.toLowerCase());

// Replace Tests
const tests = path.join(toolName, 'test', 'sample.test.js');
replaceContents(tests, 'Sample', toolName);
replaceContents(tests, 'sample', toolName.toLowerCase());

console.log(`
✅ New tool ${toolName} created in ${targetFolder}

🛠️  To build the tool, run \`npm i && npm run build\` in ${targetFolder}

🧪 To test the tool, run \`npm run test\` in ${targetFolder}
`);

