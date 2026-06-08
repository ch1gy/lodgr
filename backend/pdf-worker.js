// pdf-worker.js — reads HTML from stdin, writes A4 PDF bytes to stdout.
// Spawned by the Rust backend; no HTTP, no ports.
const path       = require('path');
const puppeteer  = require(path.resolve(__dirname, '../../Contract/node_modules/puppeteer'));

let html = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', (chunk) => { html += chunk; });
process.stdin.on('end', () => {
  (async () => {
    const browser = await puppeteer.launch({
      args: ['--no-sandbox', '--disable-setuid-sandbox'],
    });
    const page = await browser.newPage();
    await page.setContent(html, { waitUntil: 'networkidle0' });
    const pdf = await page.pdf({
      format:          'A4',
      printBackground: true,
      margin:          { top: '0', bottom: '0', left: '0', right: '0' },
    });
    await browser.close();
    process.stdout.write(pdf);
  })().catch((err) => {
    process.stderr.write(err.message + '\n');
    process.exit(1);
  });
});
