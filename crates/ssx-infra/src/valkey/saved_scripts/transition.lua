-- W00 additive authority. F07 source-exact scripts remain untouched.
-- KEYS record, definitions, due, tenants, count, recent, lease, deleted, baseline, reports.
-- ARGV mode, tenant, id, expected_raw, next_raw, quota, token, maximum_lifetime, literal_scan_pattern, exact_delete_revision, baseline_raw, baseline_presence, baseline_expiry, proof_attempt, then exact proof key/raw pairs.
local expected_types = {'string','zset','zset','zset','string','zset','string','string','string','zset'}
for i, key in ipairs(KEYS) do
  local kind = redis.call('TYPE', key).ok
  if kind ~= 'none' and kind ~= expected_types[i] then return {-4, ''} end
end
local instant = redis.call('TIME')
local now = tonumber(instant[1]) + tonumber(instant[2]) / 1000000
local mode, tenant, id = ARGV[1], ARGV[2], ARGV[3]
local prefix = 'mcp:saved:v1:definition:'
local function finite(value) return type(value) == 'number' and value == value and value ~= math.huge and value ~= -math.huge end
local function retained_ttl(expiry)
  -- Stay below Valkey signed-millisecond overflow, with conservative room for
  -- floating-point conversion. Validate before any command can change state.
  local delta = expiry - now
  if not finite(delta) or expiry > 9000000000000000 - 300 then return nil end
  return math.max(1, math.floor(delta) + 300)
end
local function record(raw, key, exact_revision_pair)
  local ok, value = pcall(cjson.decode, raw)
  if not ok or type(value) ~= 'table' or type(value.tenant) ~= 'string' or type(value.search_id) ~= 'string' or value.search_id == '' or string.find(value.search_id, ':', 1, true) then return nil end
  if prefix .. value.tenant .. ':' .. value.search_id ~= key then return nil end
  if not finite(value.expires_at) or not finite(value.next_due) or not finite(value.created_at) or not finite(value.revision) or type(value.paused) ~= 'boolean' or type(value.deleted) ~= 'boolean' then return nil end
  for _, name in ipairs({'revision','interval_seconds','retention_seconds','max_results','max_reports'}) do
    local number = value[name]
    if not finite(number) or number < 1 or (number >= 9223372036854775808 and not (name == 'revision' and exact_revision_pair)) or number ~= math.floor(number) then return nil end
  end
  if not finite(value.last_slot) or value.last_slot < -1 or value.last_slot >= 9223372036854775808 then return nil end
  return value
end
local raw = redis.call('GET', KEYS[1])
if raw and #raw > 4194304 then return {-5, ''} end
if #ARGV[5] > 4194304 then return {-5, ''} end
local retained_path = mode == 'update' or mode == 'delete' or mode == 'renew' or mode == 'release'
if retained_path and raw and raw ~= ARGV[4] then return {-2, ''} end
-- Duplicate creation publishes no authority. Repair validates this key with all
-- other scanned definitions rather than treating its placeholder ID specially.
if mode == 'create' and raw then return {0, ''} end
local current = mode ~= 'repair' and raw and record(raw, KEYS[1], retained_path and raw == ARGV[4]) or nil
if mode ~= 'repair' and raw and (not current or current.tenant ~= tenant or current.search_id ~= id) then return {-4, ''} end
if mode == 'create' or mode == 'repair' then
  local next = mode == 'create' and record(ARGV[5], KEYS[1]) or {expires_at=now}
  if mode == 'create' and (not next or next.tenant ~= tenant or next.search_id ~= id or next.revision ~= 1 or next.deleted or next.paused or next.created_at > now or next.expires_at <= now or next.expires_at - next.created_at > tonumber(ARGV[8]) or next.next_due < next.created_at or next.next_due > next.created_at + next.interval_seconds) then return {-4, ''} end
  -- Scan and validate complete bounded authority before publishing ANY repair.
  local indexed = redis.call('ZCOUNT', KEYS[2], '(' .. string.format('%.17g', now), '+inf')
  local count = tonumber(redis.call('GET', KEYS[5]))
  local repair = mode == 'repair' or not count or count < 0 or count ~= math.floor(count) or count ~= indexed
  local cursor, pages, visited, live, raw_bytes = '0', 0, 0, {}, 0
  local proofs, needed, needed_keys = {}, {}, {}
  if ARGV[14] ~= '0' and ARGV[14] ~= '1' then return {-4, ''} end
  if (#ARGV - 14) % 2 ~= 0 or #ARGV > 14 + 8192 then return {-5, ''} end
  local proof_bytes = 0
  for i = 15, #ARGV, 2 do
    if #ARGV[i+1] > 4194304 or proof_bytes > 8388608 - #ARGV[i+1] then return {-5, ''} end
    proof_bytes = proof_bytes + #ARGV[i+1]
    if proofs[ARGV[i]] then return {-4, ''} end
    proofs[ARGV[i]] = ARGV[i+1]
  end
  for key, bytes in pairs(proofs) do
    if redis.call('TYPE', key).ok ~= 'string' or redis.call('GET', key) ~= bytes then return {-2, ''} end
  end
  if repair then
  count = 0
  repeat
    local page = redis.call('SCAN', cursor, 'MATCH', ARGV[9], 'COUNT', 256)
    cursor, pages = page[1], pages + 1
    for _, key in ipairs(page[2]) do
      visited = visited + 1
      if visited > 4096 then return {-5, ''} end
      if redis.call('TYPE', key).ok ~= 'string' then return {-4, ''} end
      local bytes = redis.call('GET', key)
      if bytes then
        if #bytes > 4194304 or raw_bytes > 8388608 - #bytes then return {-5, ''} end
        raw_bytes = raw_bytes + #bytes
        -- cjson cannot distinguish valid MAX from true overflow. Provisional
        -- decode never publishes: exact raw Rust proof is mandatory below.
        local item = record(bytes, key, true)
        if not item then return {-4, ''} end
        if item.tenant == tenant and item.revision >= 9223372036854775808 and proofs[key] ~= bytes then
          if ARGV[14] == '1' then return {-2, ''} end
          if not needed_keys[key] then table.insert(needed, {key, bytes}); needed_keys[key] = true end
        end
        if item.tenant == tenant and not item.deleted and item.expires_at > now and not live[item.search_id] then
          live[item.search_id] = {expires_at=item.expires_at,next_due=item.next_due,paused=item.paused}
          count = count + 1
        end
      end
    end
    if pages >= 64 and cursor ~= '0' then return {-5, ''} end
  until cursor == '0'
  end
  if #needed > 0 then return {-6, needed} end
  if mode == 'create' and count >= tonumber(ARGV[6]) then return {-3, ''} end
  local horizon = next.expires_at
  local last = redis.call('ZRANGE', KEYS[2], -1, -1, 'WITHSCORES')
  if not repair and #last == 2 then
    if not finite(tonumber(last[2])) then return {-4, ''} end
    horizon = math.max(horizon, tonumber(last[2]))
  end
  for _, item in pairs(live) do horizon = math.max(horizon, item.expires_at) end
  local ttl = retained_ttl(horizon)
  local next_ttl = retained_ttl(next.expires_at)
  if not ttl or not next_ttl then return {-4, ''} end
  if repair then redis.call('DEL', KEYS[2], KEYS[3])
  else redis.call('ZREMRANGEBYSCORE', KEYS[2], '-inf', now) end
  for name, item in pairs(live) do
    redis.call('ZADD', KEYS[2], item.expires_at, name)
    if not item.paused then redis.call('ZADD', KEYS[3], item.next_due, name) end
  end
  if mode == 'create' then
  redis.call('SETEX', KEYS[1], next_ttl, ARGV[5])
  redis.call('ZADD', KEYS[2], next.expires_at, id)
  redis.call('ZADD', KEYS[3], next.next_due, id)
  end
  redis.call('ZADD', KEYS[4], 'GT', horizon, tenant)
  redis.call('SETEX', KEYS[5], ttl, count + (mode == 'create' and 1 or 0))
  redis.call('EXPIRE', KEYS[2], ttl)
  redis.call('EXPIRE', KEYS[3], ttl)
  if mode == 'repair' then return {1, tostring(count)} end
  redis.call('ZADD', KEYS[6], next.created_at, id)
  local excess = redis.call('ZCARD', KEYS[6]) - 200
  if excess > 0 then redis.call('ZREMRANGEBYRANK', KEYS[6], 0, excess - 1) end
  if redis.call('TTL', KEYS[6]) < ttl then redis.call('EXPIRE', KEYS[6], ttl) end
  return {1, ARGV[5]}
end
if not current or current.deleted or current.expires_at <= now then return {-1, ''} end
if raw ~= ARGV[4] then return {-2, ''} end
local current_ttl = retained_ttl(current.expires_at)
if not current_ttl then return {-4, ''} end
if mode == 'claim' then
  local baseline = redis.call('GET', KEYS[9])
  if (ARGV[12] == 'missing' and baseline ~= false) or (ARGV[12] == 'exact' and baseline ~= ARGV[11]) then return {-2, ''} end
  local baseline_expiry = tonumber(ARGV[13])
  if not finite(baseline_expiry) then return {-4, ''} end
  if current.paused or current.next_due > now or redis.call('EXISTS', KEYS[7]) ~= 0 then return {0, ''} end
  local next = record(ARGV[5], KEYS[1])
  if not next or next.revision ~= current.revision or next.lease_token ~= ARGV[7] or ARGV[7] == '' then return {-4, ''} end
  -- Preserve the source cjson claim rewrite: its rounded durable next_due is
  -- part of run identity. Original raw snapshot remains the CAS authority.
  current.lease_token = ARGV[7]
  current.lease_expires_at = next.lease_expires_at
  local encoded, committed = pcall(cjson.encode, current)
  if not encoded then return {-4, ''} end
  if #committed > 4194304 then return {-5, ''} end
  local durable = record(committed, KEYS[1])
  if not durable then return {-4, ''} end
  if durable.expires_at <= now then return {-1, ''} end
  redis.call('SET', KEYS[7], ARGV[7], 'EX', 60)
  redis.call('SETEX', KEYS[1], current_ttl, committed)
  return {baseline_expiry > now and 2 or 1, committed}
end
if mode == 'renew' or mode == 'release' then
  if redis.call('GET', KEYS[7]) ~= ARGV[7] then return {0, ''} end
  if mode == 'renew' then redis.call('EXPIRE', KEYS[7], 60) else redis.call('DEL', KEYS[7]) end
  return {1, ''}
end
if mode == 'update' then
  local next = record(ARGV[5], KEYS[1], true)
  if not next or next.tenant ~= tenant or next.search_id ~= id or next.expires_at ~= current.expires_at or next.created_at > now or next.deleted then return {-4, ''} end
  redis.call('SETEX', KEYS[1], current_ttl, ARGV[5])
  if next.paused then redis.call('ZREM', KEYS[3], id) else redis.call('ZADD', KEYS[3], next.next_due, id) end
  redis.call('DEL', KEYS[7], KEYS[9])
  redis.call('ZADD', KEYS[6], next.created_at, id)
  local excess = redis.call('ZCARD', KEYS[6]) - 200
  if excess > 0 then redis.call('ZREMRANGEBYRANK', KEYS[6], 0, excess - 1) end
  local ttl = current_ttl
  if redis.call('TTL', KEYS[6]) < ttl then redis.call('EXPIRE', KEYS[6], ttl) end
  return {1, ARGV[5]}
end
if mode == 'delete' then
  if ARGV[10] == '' or tonumber(ARGV[10]) ~= current.revision then return {-2, ''} end
  local reports = redis.call('ZRANGE', KEYS[10], 0, 4096)
  if #reports > 4096 then return {-5, ''} end
  local report_prefix = 'mcp:saved:v1:report:' .. tenant .. ':' .. id .. ':'
  for _, run in ipairs(reports) do
    local kind = redis.call('TYPE', report_prefix .. run).ok
    if kind ~= 'none' and kind ~= 'string' then return {-4, ''} end
  end
  redis.call('DEL', KEYS[1], KEYS[5], KEYS[7], KEYS[9], KEYS[10])
  redis.call('ZREM', KEYS[2], id)
  redis.call('ZREM', KEYS[3], id)
  redis.call('ZREM', KEYS[6], id)
  redis.call('SETEX', KEYS[8], current_ttl, ARGV[10])
  for _, run in ipairs(reports) do redis.call('DEL', report_prefix .. run) end
  return {1, ''}
end
return {-4, ''}
