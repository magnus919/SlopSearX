-- W04 atomic fenced publication. Rust supplies exact validated bytes; Lua never
-- reserializes durable numbers. No outbox publication (W05/W09 composition).
-- KEYS definition, lease, report, reports, due, baseline
-- ARGV expected_raw, token, fingerprint, next_raw, report_raw, baseline_raw,
-- definition_expiry, report_expiry, baseline_expiry, finished_at, run_id,
-- max_reports, report_prefix, next_due, search_id
local kinds = {'string','string','string','zset','zset','string'}
for i, key in ipairs(KEYS) do
  local kind = redis.call('TYPE', key).ok
  if kind ~= 'none' and kind ~= kinds[i] then return -4 end
end
local raw = redis.call('GET', KEYS[1])
if not raw or raw ~= ARGV[1] or redis.call('GET', KEYS[2]) ~= ARGV[2] then return 0 end
local time = redis.call('TIME')
local now = tonumber(time[1]) + tonumber(time[2]) / 1000000
local function ttl(value)
  local expiry = tonumber(value)
  if not expiry or expiry ~= expiry or expiry > 9000000000000000 - 300 then return nil end
  return math.max(1, math.floor(expiry-now) + 300)
end
local definition_ttl, report_ttl, baseline_ttl = ttl(ARGV[7]), ttl(ARGV[8]), ttl(ARGV[9])
if not definition_ttl or not report_ttl or not baseline_ttl then return -4 end
if tonumber(ARGV[7]) <= now then return 0 end
-- Rust preflight validates exact revision, active state, binding, and fingerprint
-- against expected_raw. Exact CAS proves these facts are still current.
local max_reports = tonumber(ARGV[12])
if not max_reports or max_reports < 1 or max_reports > 100 then return -4 end
local previous_reports_ttl = redis.call('TTL', KEYS[4])
local previous_due_ttl = redis.call('TTL', KEYS[5])
local existing = redis.call('ZRANGE', KEYS[4], 0, 4096)
if #existing > 4096 then return -4 end
for _, run in ipairs(existing) do
  local kind = redis.call('TYPE', ARGV[13] .. run).ok
  if kind ~= 'none' and kind ~= 'string' then return -4 end
end
redis.call('SETEX', KEYS[3], report_ttl, ARGV[5])
redis.call('ZADD', KEYS[4], ARGV[10], ARGV[11])
if previous_reports_ttl == -2 or (previous_reports_ttl >= 0 and previous_reports_ttl < report_ttl) then redis.call('EXPIRE', KEYS[4], report_ttl) end
while redis.call('ZCARD', KEYS[4]) > max_reports do
  local oldest = redis.call('ZRANGE', KEYS[4], 0, 0)[1]
  redis.call('ZREM', KEYS[4], oldest)
  redis.call('DEL', ARGV[13] .. oldest)
end
redis.call('SETEX', KEYS[1], definition_ttl, ARGV[4])
redis.call('ZADD', KEYS[5], ARGV[14], ARGV[15])
-- Shared indexes must not expire before already retained members. W00 repair
-- also uses the maximum authoritative definition horizon.
if previous_due_ttl == -2 or (previous_due_ttl >= 0 and previous_due_ttl < definition_ttl) then redis.call('EXPIRE', KEYS[5], definition_ttl) end
if ARGV[6] ~= '' then redis.call('SETEX', KEYS[6], baseline_ttl, ARGV[6]) end
redis.call('DEL', KEYS[2])
return 1
