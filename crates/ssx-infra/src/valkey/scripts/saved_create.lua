
redis.call('ZREMRANGEBYSCORE', KEYS[2], '-inf', ARGV[1])
if redis.call('EXISTS', KEYS[1]) == 1 then return 0 end
local authoritative = tonumber(redis.call('GET', KEYS[5]))
local indexed = redis.call('ZCARD', KEYS[2])
if not authoritative or authoritative ~= indexed then
  local known_tenant = redis.call('ZSCORE', KEYS[4], ARGV[10])
  if indexed == 0 and not authoritative and not known_tenant and redis.call('EXISTS', KEYS[4]) == 1 then
    authoritative = 0
  else
    local page = redis.call('SCAN', '0', 'MATCH', ARGV[9], 'COUNT', 256)
    local found = {}
    authoritative = 0
    for _, key in ipairs(page[2]) do
      local raw = redis.call('GET', key)
      if raw then
        local decoded, definition = pcall(cjson.decode, raw)
        if not decoded then return -2 end
        if definition.tenant == ARGV[10] and not definition.deleted and
           tonumber(definition.expires_at) > tonumber(ARGV[1]) then
          authoritative = authoritative + 1
          found[definition.search_id] = tonumber(definition.expires_at)
        end
      end
    end
    if authoritative >= tonumber(ARGV[2]) then return -1 end
    if page[1] ~= '0' then return -2 end
    redis.call('DEL', KEYS[2])
    for search_id, expires_at in pairs(found) do redis.call('ZADD', KEYS[2], expires_at, search_id) end
  end
end
if authoritative >= tonumber(ARGV[2]) then return -1 end
redis.call('SETEX', KEYS[1], ARGV[3], ARGV[4])
redis.call('ZADD', KEYS[2], ARGV[5], ARGV[6])
redis.call('ZADD', KEYS[3], ARGV[7], ARGV[6])
redis.call('ZADD', KEYS[4], 'GT', ARGV[5], ARGV[8])
if redis.call('TTL', KEYS[2]) < tonumber(ARGV[3]) then redis.call('EXPIRE', KEYS[2], ARGV[3]) end
if redis.call('TTL', KEYS[3]) < tonumber(ARGV[3]) then redis.call('EXPIRE', KEYS[3], ARGV[3]) end
local count_ttl = redis.call('TTL', KEYS[5])
redis.call('SET', KEYS[5], authoritative + 1)
redis.call('EXPIRE', KEYS[5], math.max(count_ttl, tonumber(ARGV[3])))
return 1
