
local raw = redis.call('GET', KEYS[1])
if not raw then return -1 end
local current = cjson.decode(raw)
if current.deleted or tonumber(current.expires_at) <= tonumber(ARGV[1]) then return -1 end
if tonumber(current.revision) ~= tonumber(ARGV[2]) then return tonumber(current.revision) end
if ARGV[8] ~= '' then
  redis.call('XTRIM', KEYS[5], 'MINID', ARGV[9])
  if redis.call('XLEN', KEYS[5]) >= tonumber(ARGV[10]) then return -3 end
  redis.call('XADD', KEYS[5], '*', 'payload', ARGV[8])
  redis.call('EXPIRE', KEYS[5], ARGV[11])
end
redis.call('SETEX', KEYS[1], ARGV[3], ARGV[4])
if ARGV[5] == 'paused' then redis.call('ZREM', KEYS[2], ARGV[6])
else redis.call('ZADD', KEYS[2], ARGV[7], ARGV[6]) end
redis.call('DEL', KEYS[3])
redis.call('DEL', KEYS[4])
return 0
