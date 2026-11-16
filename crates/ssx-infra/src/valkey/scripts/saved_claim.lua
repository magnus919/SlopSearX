
local raw = redis.call('GET', KEYS[1])
if not raw then return 0 end
local current = cjson.decode(raw)
if current.deleted or current.paused or tonumber(current.expires_at) <= tonumber(ARGV[1]) or
   tonumber(current.next_due) > tonumber(ARGV[1]) then return 0 end
if redis.call('SET', KEYS[2], ARGV[2], 'NX', 'EX', ARGV[3]) == false then return 0 end
current.lease_token = ARGV[2]
current.lease_expires_at = tonumber(ARGV[1]) + tonumber(ARGV[3])
redis.call('SETEX', KEYS[1], ARGV[4], cjson.encode(current))
return cjson.encode(current)
