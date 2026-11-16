-- W05 event publication extends every accepted W04 fence, with exact durable bytes.
-- KEYS[7] events; ARGV[16..18] payload, retention_seconds, capacity.
-- Rust validates typed publication arguments; Lua validates mutable store types,
-- ownership, TTLs, report eviction keys, and stream preconditions before mutation.
-- Lua errors do not roll back.
-- KEYS definition, lease, report, reports, due, baseline
-- ARGV expected_raw, token, fingerprint, next_raw, report_raw, baseline_raw,
-- definition_expiry, report_expiry, baseline_expiry, finished_at, run_id,
-- max_reports, report_prefix, next_due, search_id
local kinds = {'string','string','string','zset','zset','string','stream'}
for i, key in ipairs(KEYS) do
  local kind = redis.call('TYPE', key).ok
  if kind ~= 'none' and kind ~= kinds[i] then return {-4,''} end
end
local raw = redis.call('GET', KEYS[1])
if not raw or raw ~= ARGV[1] or redis.call('GET', KEYS[2]) ~= ARGV[2] then return {0,''} end
local time = redis.call('TIME')
local now = tonumber(time[1]) + tonumber(time[2]) / 1000000
local function ttl(value)
  local expiry = tonumber(value)
  if not expiry or expiry ~= expiry or expiry > 9000000000000000 - 300 then return nil end
  return math.max(1, math.floor(expiry-now) + 300)
end
local definition_ttl, report_ttl, baseline_ttl = ttl(ARGV[7]), ttl(ARGV[8]), ttl(ARGV[9])
if not definition_ttl or not report_ttl or not baseline_ttl then return {-4,''} end
if tonumber(ARGV[7]) <= now then return {0,''} end
-- Rust preflight validates exact revision, active state, binding, and fingerprint
-- against expected_raw. Exact CAS proves these facts are still current.
local max_reports = tonumber(ARGV[12])
if not max_reports or max_reports < 1 or max_reports > 100 then return {-4,''} end
local previous_reports_ttl = redis.call('TTL', KEYS[4])
local previous_due_ttl = redis.call('TTL', KEYS[5])
local existing = redis.call('ZRANGE', KEYS[4], 0, 4096)
if #existing > 4096 then return {-4,''} end
for _, run in ipairs(existing) do
  local kind = redis.call('TYPE', ARGV[13] .. run).ok
  if kind ~= 'none' and kind ~= 'string' then return {-4,''} end
end
local retention, capacity = tonumber(ARGV[17]), tonumber(ARGV[18])
if not retention or retention % 1 ~= 0 or retention < 1 or retention > 31536000 or not capacity or capacity % 1 ~= 0 or capacity < 1 or capacity > 100000 or #ARGV[16] == 0 or #ARGV[16] > 65536 then return {-4,''} end
-- A malicious/corrupt extreme last-generated-id must not make XADD fail after
-- report writes. XINFO is read-only and also observes an emptied stream's ID.
if redis.call('EXISTS', KEYS[7]) == 1 then
  local info = redis.call('XINFO', 'STREAM', KEYS[7])
  for i = 1, #info, 2 do
    if info[i] == 'last-generated-id' then
      if info[i+1] == '18446744073709551615-18446744073709551615' then return {-4,''} end
    end
  end
end
local cutoff = string.format('%.0f', math.max(0, math.floor((now-retention)*1000))) .. '-0'
-- Source-compatible trim-only side effect on a capacity rejection. Every key,
-- argument, report-eviction target, and stream-generation precondition is checked.
redis.call('XTRIM', KEYS[7], 'MINID', cutoff)
if redis.call('XLEN', KEYS[7]) >= capacity then return {-5,''} end
local cursor = redis.call('XADD', KEYS[7], '*', 'payload', ARGV[16])
redis.call('EXPIRE', KEYS[7], retention+300)
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
return {1,cursor}
