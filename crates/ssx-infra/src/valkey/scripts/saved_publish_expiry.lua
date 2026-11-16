
if redis.call('EXISTS', KEYS[2]) == 1 then return 0 end
local raw = redis.call('GET', KEYS[1])
if not raw then return 0 end
local current = cjson.decode(raw)
if tonumber(current.expires_at) > tonumber(ARGV[1]) then return 0 end
redis.call('XTRIM', KEYS[3], 'MINID', ARGV[2])
if redis.call('XLEN', KEYS[3]) >= tonumber(ARGV[3]) then return -1 end
redis.call('XADD', KEYS[3], '*', 'payload', ARGV[4])
redis.call('EXPIRE', KEYS[3], ARGV[5])
redis.call('SETEX', KEYS[2], ARGV[5], '1')
return 1
