import { createReadStream, readdirSync, statSync, existsSync } from 'node:fs'
import { createInterface } from 'node:readline'
import { join } from 'node:path'
import { homedir } from 'node:os'

const projectsDir = join(homedir(), '.claude', 'projects')
const since = Date.now() - 24 * 3600 * 1000
const tokenFields = ['input_tokens', 'output_tokens', 'cache_creation_input_tokens', 'cache_read_input_tokens']
const suspicious = /cost|usd|dollar|rate.?limit|quota|utiliz|five_hour|seven_day/i
const opaque = new Set(['wireToolInputs', 'snapshot', 'toolUseResult', 'content', 'input', 'params', 'base', 'keep', 'tail'])
const assistantKeys = new Set()
const usageKeys = new Set()
const messageKeys = new Set()
const suspiciousKeys = new Map()
let bytes = 0

const collectKeys = (value, path, depth) => {
  if (!value || typeof value !== 'object' || depth > 4) return
  for (const [key, child] of Object.entries(value)) {
    if (suspicious.test(key)) suspiciousKeys.set(`${path}.${key}`, (suspiciousKeys.get(`${path}.${key}`) ?? 0) + 1)
    if (!Array.isArray(value) && !opaque.has(key)) collectKeys(child, `${path}.${key}`, depth + 1)
  }
}

const scanFile = async (file) => {
  bytes += statSync(file).size
  const stats = { lines: 0, types: {}, models: new Set(), usageLines: 0, ids: new Map(), sums: Object.fromEntries(tokenFields.map((f) => [f, 0])), last: null, first: null, end: null, sidechain: 0, agentIdLines: 0, badLines: 0, entrypoint: null, costStates: 0, lastCostUSD: null, quotaLines: 0 }
  for await (const line of createInterface({ input: createReadStream(file), crlfDelay: Infinity })) {
    if (!line) continue
    stats.lines++
    let entry
    try { entry = JSON.parse(line) } catch { stats.badLines++; continue }
    stats.types[entry.type] = (stats.types[entry.type] ?? 0) + 1
    if (entry.timestamp) { stats.first ??= entry.timestamp; stats.end = entry.timestamp }
    if (entry.isSidechain) stats.sidechain++
    if (entry.agentId) stats.agentIdLines++
    stats.entrypoint ??= entry.entrypoint ?? null
    if (entry.quotaLimits) stats.quotaLines++
    if (entry.type === 'cost-state') { stats.costStates++; stats.lastCostUSD = entry.totalCostUSD }
    const { message, ...rest } = entry
    collectKeys(rest, entry.type, 0)
    if (message && typeof message === 'object') {
      const { content, ...meta } = message
      collectKeys(meta, `${entry.type}.message`, 0)
    }
    if (entry.type !== 'assistant' || !message) continue
    Object.keys(entry).forEach((k) => assistantKeys.add(k))
    Object.keys(message).forEach((k) => messageKeys.add(k))
    if (message.model) stats.models.add(message.model)
    if (!message.usage) continue
    Object.keys(message.usage).forEach((k) => usageKeys.add(k))
    stats.usageLines++
    for (const f of tokenFields) stats.sums[f] += message.usage[f] ?? 0
    stats.last = Object.fromEntries(tokenFields.map((f) => [f, message.usage[f] ?? 0]))
    const seen = stats.ids.get(message.id) ?? { count: 0, usages: new Set(), lastUsage: null }
    seen.count++
    seen.usages.add(JSON.stringify(tokenFields.map((f) => message.usage[f] ?? 0)))
    seen.lastUsage = message.usage
    stats.ids.set(message.id, seen)
  }
  const repeated = [...stats.ids.values()].filter((s) => s.count > 1)
  const deduped = Object.fromEntries(tokenFields.map((f) => [f, [...stats.ids.values()].reduce((a, s) => a + (s.lastUsage[f] ?? 0), 0)]))
  return { ...stats, models: [...stats.models], uniqueIds: stats.ids.size, repeatedIds: repeated.length, repeatedLines: repeated.reduce((a, s) => a + s.count, 0), repeatedWithDifferentUsage: repeated.filter((s) => s.usages.size > 1).length, deduped, ids: undefined }
}

const started = performance.now()
const rows = []
for (const project of readdirSync(projectsDir, { withFileTypes: true }).filter((d) => d.isDirectory())) {
  const dir = join(projectsDir, project.name)
  for (const name of readdirSync(dir).filter((n) => n.endsWith('.jsonl'))) {
    if (statSync(join(dir, name)).mtimeMs < since) continue
    const sessionId = name.slice(0, -6)
    const main = await scanFile(join(dir, name))
    const subDir = join(dir, sessionId, 'subagents')
    const subFiles = existsSync(subDir) ? readdirSync(subDir).filter((n) => n.endsWith('.jsonl')) : []
    const subs = []
    for (const f of subFiles) subs.push(await scanFile(join(subDir, f)))
    rows.push({ session: sessionId.slice(0, 8), project: project.name.slice(0, 48), main, subagentFiles: subFiles.length, subagentLines: subs.reduce((a, s) => a + s.lines, 0), subagentSidechainLines: subs.reduce((a, s) => a + s.sidechain, 0), subagentUsageLines: subs.reduce((a, s) => a + s.usageLines, 0), subagentDeduped: Object.fromEntries(tokenFields.map((f) => [f, subs.reduce((a, s) => a + s.deduped[f], 0)])), subagentRepeatedIds: subs.reduce((a, s) => a + s.repeatedIds, 0) })
  }
}
const elapsedMs = performance.now() - started

for (const r of rows) {
  const m = r.main
  console.log([
    `${r.session} ${r.project} entrypoint=${m.entrypoint} costStateLines=${m.costStates} lastTotalCostUSD=${m.lastCostUSD} quotaLimitLines=${m.quotaLines}`,
    `  lines=${m.lines} bad=${m.badLines} types=${JSON.stringify(m.types)}`,
    `  models=${m.models.join(',')} usageLines=${m.usageLines} uniqueMsgIds=${m.uniqueIds} repeatedIds=${m.repeatedIds} (lines ${m.repeatedLines}, differingUsage ${m.repeatedWithDifferentUsage})`,
    `  naiveSum=${JSON.stringify(m.sums)}`,
    `  dedupedSum=${JSON.stringify(m.deduped)}`,
    `  lastUsage=${JSON.stringify(m.last)} contextProxy=${m.last ? m.last.input_tokens + m.last.cache_creation_input_tokens + m.last.cache_read_input_tokens : 0}`,
    `  first=${m.first} last=${m.end}`,
    `  mainSidechainLines=${m.sidechain} mainAgentIdLines=${m.agentIdLines} subagentFiles=${r.subagentFiles} subLines=${r.subagentLines} subSidechain=${r.subagentSidechainLines} subUsageLines=${r.subagentUsageLines} subRepeatedIds=${r.subagentRepeatedIds} subDeduped=${JSON.stringify(r.subagentDeduped)}`
  ].join('\n'))
}
console.log('\nassistant line keys:', [...assistantKeys].sort().join(', '))
console.log('assistant message keys:', [...messageKeys].sort().join(', '))
console.log('message.usage keys:', [...usageKeys].sort().join(', '))
console.log('cost/rate-limit-like key paths (count):', JSON.stringify(Object.fromEntries(suspiciousKeys)))
console.log(`sessions=${rows.length} MB=${(bytes / 1048576).toFixed(1)} elapsedMs=${elapsedMs.toFixed(0)}`)
