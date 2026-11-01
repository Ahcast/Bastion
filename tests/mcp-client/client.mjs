import assert from 'node:assert/strict';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StreamableHTTPClientTransport } from '@modelcontextprotocol/sdk/client/streamableHttp.js';

const deadline = setTimeout(() => process.exit(1), 15000);
deadline.unref();
const transport = new StreamableHTTPClientTransport(new URL(process.env.MCP_FIXTURE_ENDPOINT), {
  requestInit: { headers: { Authorization: `Bearer ${process.env.MCP_FIXTURE_ACCESS}` } }
});
const client = new Client({ name: 'bastion-interop-fixture', version: '1.0.0' });
try {
  await client.connect(transport);
  const list = await client.listTools();
  assert(list.tools.some(tool => tool.name === 'cotizaciones.listar'));
  assert(!list.tools.some(tool => tool.name.startsWith('facturas.')));
  const result = await client.callTool({ name: 'cotizaciones.listar', arguments: { page_size: 5 } });
  assert.equal(result.isError, false);
  assert.equal(result.structuredContent.page_size, 5);
  console.log('SDK MCP: inicialización, herramientas y consulta correctas');
} finally {
  await client.close();
}
