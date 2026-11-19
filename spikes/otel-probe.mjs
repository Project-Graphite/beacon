import { createServer } from 'node:http'
import { spawn } from 'node:child_process'
import { mkdirSync, writeFileSync } from 'node:fs'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

const outDir = join(dirname(fileURLToPath(import.meta.url)), 'out', 'usage')
const captureDir = join(outDir, 'otel')
const workDir = join(outDir, 'work')
mkdirSync(captureDir, { recursive: true })
mkdirSync(workDir, { recursive: true })

const requests = []
const server = createServer((req, res) => {
  const chunks = []
  req.on('data', (c) => chunks.push(c))
  req.on('end', () => {
    const raw = Buffer.concat(chunks).toString('utf8')
    let body = raw
    try { body = JSON.parse(raw) } catch {}
    const record = { at: new Date().toISOString(), method: req.method, path: req.url, contentType: req.headers['content-type'], bytes: raw.length, body }
    requests.push(record)
    writeFileSync(join(captureDir, `${String(requests.length).padStart(3, '0')}.json`), JSON.stringify(record, null, 1))
    res.writeHead(200, { 'content-type': 'application/json' })
    res.end('{}')
  })
})
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve))
const endpoint = `http://127.0.0.1:${server.address().port}`

const otelEnv = {
  CLAUDE_CODE_ENABLE_TELEMETRY: '1',
  OTEL_METRICS_EXPORTER: 'otlp',
  OTEL_LOGS_EXPORTER: 'otlp',
  OTEL_EXPORTER_OTLP_PROTOCOL: 'http/json',
  OTEL_EXPORTER_OTLP_ENDPOINT: endpoint,
  OTEL_METRIC_EXPORT_INTERVAL: '1000',
  OTEL_LOGS_EXPORT_INTERVAL: '1000',
  OTEL_METRICS_INCLUDE_ENTRYPOINT: 'true',
  OTEL_METRICS_INCLUDE_VERSION: 'true'
}
const env = { ...Object.fromEntries(Object.entries(process.env).filter(([k]) => !/^(CLAUDE|ANTHROPIC)/i.test(k))), ...otelEnv }

const stderr = []
const started = Date.now()
const exitCode = await new Promise((resolve) => {
  const child = spawn('claude', ['-p', '--model', 'haiku', 'Reply with the single word OK.'], { cwd: workDir, env, stdio: ['ignore', 'pipe', 'pipe'] })
  child.stdout.on('data', () => {})
  child.stderr.on('data', (c) => stderr.push(c))
  child.on('exit', resolve)
})
const exitedAfterMs = Date.now() - started
await new Promise((resolve) => setTimeout(resolve, 5000))
server.close()

const attrKeys = (attrs) => (attrs ?? []).map((a) => a.key).sort()
const metrics = {}
const events = {}
const resourceKeys = new Set()
for (const { path, body } of requests) {
  if (typeof body !== 'object') continue
  for (const rm of body.resourceMetrics ?? []) {
    attrKeys(rm.resource?.attributes).forEach((k) => resourceKeys.add(`metrics:${k}`))
    for (const sm of rm.scopeMetrics ?? []) for (const m of sm.metrics ?? []) {
      const points = (m.sum ?? m.gauge ?? m.histogram)?.dataPoints ?? []
      metrics[m.name] ??= { unit: m.unit, path, points: 0, attributeKeys: new Set(), values: [] }
      for (const p of points) {
        metrics[m.name].points++
        attrKeys(p.attributes).forEach((k) => metrics[m.name].attributeKeys.add(k))
        metrics[m.name].values.push({ value: p.asDouble ?? p.asInt, type: p.attributes?.find((a) => a.key === 'type')?.value?.stringValue, model: p.attributes?.find((a) => a.key === 'model')?.value?.stringValue })
      }
    }
  }
  for (const rl of body.resourceLogs ?? []) {
    attrKeys(rl.resource?.attributes).forEach((k) => resourceKeys.add(`logs:${k}`))
    for (const sl of rl.scopeLogs ?? []) for (const r of sl.logRecords ?? []) {
      const name = r.attributes?.find((a) => a.key === 'event.name')?.value?.stringValue ?? r.body?.stringValue ?? 'unnamed'
      events[name] ??= { path, count: 0, body: r.body?.stringValue, attributeKeys: new Set() }
      events[name].count++
      attrKeys(r.attributes).forEach((k) => events[name].attributeKeys.add(k))
    }
  }
}
const asSorted = (o) => Object.fromEntries(Object.entries(o).map(([k, v]) => [k, { ...v, attributeKeys: [...v.attributeKeys].sort() }]))
const allKeys = [...Object.values(metrics), ...Object.values(events)].flatMap((v) => [...v.attributeKeys])
const summary = {
  endpoint,
  exitCode,
  exitedAfterMs,
  requestCount: requests.length,
  paths: [...new Set(requests.map((r) => `${r.method} ${r.path} ${r.contentType}`))],
  resourceAttributeKeys: [...resourceKeys].sort(),
  metrics: asSorted(metrics),
  events: asSorted(events),
  sessionIdPresent: allKeys.includes('session.id'),
  rateLimitOrQuotaKeys: [...new Set(allKeys.filter((k) => /rate|quota|limit|utiliz/i.test(k)))],
  otelEnv: { ...otelEnv, stripped: Object.keys(process.env).filter((k) => /^(CLAUDE|ANTHROPIC)/i.test(k)) },
  stderr: Buffer.concat(stderr).toString('utf8')
}
writeFileSync(join(outDir, 'otel-summary.json'), JSON.stringify(summary, null, 1))
console.log(JSON.stringify(summary, null, 1))
