
local existing = redis.call('GET', KEYS[2])
if existing then
  local mapped = cjson.decode(existing)
  local raw = redis.call('GET', mapped.record_key)
  if raw then
    local current = cjson.decode(raw)
    if current.plan_digest == ARGV[1] then return {'replay', raw} end
    return {'conflict', ''}
  end
end
redis.call('ZREMRANGEBYSCORE', KEYS[3], '-inf', ARGV[2])
if redis.call('ZCARD', KEYS[3]) == 0 and redis.call('EXISTS', KEYS[5]) == 1 then
  local page=redis.call('SCAN','0','MATCH',ARGV[7],'COUNT',256)
  if page[1] ~= '0' then return {'unavailable',''} end
  for _, key in ipairs(page[2]) do
    local candidate=redis.call('GET',key)
    if candidate then
      local ok,item=pcall(cjson.decode,candidate)
      if not ok then return {'unavailable',''} end
      if tonumber(item.expires_at) > tonumber(ARGV[2]) then
        redis.call('ZADD',KEYS[3],item.expires_at,item.operation_id)
      end
    end
  end
end
if redis.call('ZCARD', KEYS[3]) >= tonumber(ARGV[3]) then return {'quota', ''} end
if redis.call('EXISTS', KEYS[1]) == 1 then return {'conflict', ''} end
redis.call('SETEX', KEYS[1], ARGV[4], ARGV[5])
redis.call('SETEX', KEYS[2], ARGV[4], cjson.encode({record_key=KEYS[1]}))
redis.call('ZADD', KEYS[3], ARGV[6], ARGV[8])
redis.call('EXPIRE', KEYS[3], ARGV[4])
redis.call('SETEX', KEYS[5], ARGV[4], '1')
redis.call('ZADD', KEYS[4], ARGV[2], KEYS[1])
return {'created', ARGV[5]}
